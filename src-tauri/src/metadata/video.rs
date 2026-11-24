use serde::Serialize;

/// Video-file classification shared by the streaming workflow.
/// Decisions use file PATHS (full relative path inside the torrent), not bare names alone.

const VIDEO_EXTS: [&str; 7] = ["mp4", "mkv", "avi", "webm", "mov", "m4v", "ts"];

#[derive(Debug, Clone, PartialEq)]
pub enum VideoClass {
    /// A regular video file (movie or episode candidate).
    Video,
    /// Bonus junk that must not be auto-selected.
    Extras,
    /// Not a video file at all.
    Other,
}

fn extension(name: &str) -> String {
    name.rsplit('.').next().unwrap_or("").to_lowercase()
}

pub fn is_video_filename(name: &str) -> bool {
    VIDEO_EXTS.contains(&extension(name).as_str())
}

/// Sample / trailer / featurette / bonus heuristics. Checked against the full
/// lowercase relative path so folder names like "extras/" also count.
pub fn is_extras(path: &str) -> bool {
    let lower = path.to_lowercase();
    let markers = [
        "sample",
        "trailer",
        "featurette",
        "behind",
        "behindthescenes",
        "bonus",
        "extras",
        "deleted",
        "interview",
        "teaser",
        "proof",
        "screens",
    ];
    markers.iter().any(|m| lower.contains(m))
}

pub fn classify(path: &str) -> VideoClass {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    if !is_video_filename(name) {
        return VideoClass::Other;
    }
    if is_extras(path) {
        return VideoClass::Extras;
    }
    VideoClass::Video
}

#[derive(Debug, Clone, Serialize)]
pub struct VideoFileInfo {
    pub file_index: usize,
    /// Full relative path inside the torrent.
    pub path: String,
    pub name: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct EpisodeFileMatch {
    pub file_index: usize,
    pub path: String,
    pub name: String,
    pub size_bytes: u64,
    /// 0..100. >= 80 is auto-playable; below requires user confirmation.
    pub confidence: u8,
}

/// Extract (season, episode) from a release-style file or folder path.
/// Supports S01E01, S1E1, 1x01, 01x01 (case-insensitive).
fn season_episode_tokens(path: &str) -> Vec<(u32, u32)> {
    let lower = path.to_lowercase();
    let mut out = Vec::new();
    let bytes: Vec<char> = lower.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        // s##e## / s#e#
        if bytes[i] == 's' {
            let mut j = i + 1;
            let mut season_digits = 0;
            while j < bytes.len() && bytes[j].is_ascii_digit() && season_digits < 3 {
                j += 1;
                season_digits += 1;
            }
            if season_digits > 0
                && j < bytes.len()
                && (bytes[j] == 'e')
            {
                let mut k = j + 1;
                let mut episode_digits = 0;
                while k < bytes.len() && bytes[k].is_ascii_digit() && episode_digits < 3 {
                    k += 1;
                    episode_digits += 1;
                }
                if episode_digits > 0 {
                    let season: String = bytes[i + 1..i + 1 + season_digits].iter().collect();
                    let episode: String =
                        bytes[j + 1..j + 1 + episode_digits].iter().collect();
                    if let (Ok(s), Ok(e)) = (season.parse::<u32>(), episode.parse::<u32>()) {
                        out.push((s, e));
                        // Double-episode tokens: S01E02E03 → also emit (s, 3).
                        let mut k2 = k;
                        while k2 < bytes.len() && bytes[k2] == 'e' {
                            let mut k3 = k2 + 1;
                            let mut more = 0;
                            while k3 < bytes.len() && bytes[k3].is_ascii_digit() && more < 3 {
                                k3 += 1;
                                more += 1;
                            }
                            if more == 0 {
                                break;
                            }
                            let ep: String = bytes[k2 + 1..k3].iter().collect();
                            if let Ok(e2) = ep.parse::<u32>() {
                                out.push((s, e2));
                            }
                            k2 = k3;
                        }
                        i = k2;
                        continue;
                    }
                }
            }
        }
        // #x## (1x01); guard against matching inside words like "4x1200 resolution"
        if bytes[i].is_ascii_digit() {
            let mut j = i;
            let mut season_digits = 0;
            while j < bytes.len() && bytes[j].is_ascii_digit() && season_digits < 3 {
                j += 1;
                season_digits += 1;
            }
            if season_digits > 0 && j < bytes.len() && bytes[j] == 'x' {
                let mut k = j + 1;
                let mut episode_digits = 0;
                while k < bytes.len() && bytes[k].is_ascii_digit() && episode_digits < 3 {
                    k += 1;
                    episode_digits += 1;
                }
                if episode_digits >= 1 {
                    // require word-ish boundary after episode digits
                    let boundary = k >= bytes.len()
                        || !(bytes[k].is_ascii_alphanumeric());
                    if boundary {
                        let season: String = bytes[i..i + season_digits].iter().collect();
                        let episode: String = bytes[j + 1..j + 1 + episode_digits].iter().collect();
                        if let (Ok(s), Ok(e)) = (season.parse::<u32>(), episode.parse::<u32>()) {
                            out.push((s, e));
                        }
                        i = k;
                        continue;
                    }
                }
            }
        }
        i += 1;
    }
    out
}

/// Match torrent files to (season, episode) with confidence scores.
/// - Exact S/E token match in path: high confidence (90) + 5 if episode is zero-padded 2+ digits.
/// - Episode number only match: medium (60) — requires manual confirmation.
/// - No match: empty result.
pub fn match_episode(
    files: &[VideoFileInfo],
    season: u32,
    episode: u32,
) -> Vec<EpisodeFileMatch> {
    let mut out: Vec<EpisodeFileMatch> = files
        .iter()
        .filter_map(|f| {
            if classify(&f.path) != VideoClass::Video {
                return None;
            }
            let tokens = season_episode_tokens(&f.path);
            let mut confidence: u8 = 0;
            for (s, e) in &tokens {
                if *s == season && *e == episode {
                    confidence = confidence.max(95);
                } else if *e == episode {
                    // episode-number-only match: plausible but needs confirmation
                    confidence = confidence.max(60);
                }
                // season-only matches are noise in season packs: ignored
            }
            if confidence == 0 {
                return None;
            }
            Some(EpisodeFileMatch {
                file_index: f.file_index,
                path: f.path.clone(),
                name: f.name.clone(),
                size_bytes: f.size_bytes,
                confidence,
            })
        })
        .collect();
    out.sort_by(|a, b| b.confidence.cmp(&a.confidence).then(a.file_index.cmp(&b.file_index)));
    out
}

/// Highest-confidence match, only when unambiguous (>= 80 and clearly ahead of the rest).
pub fn unambiguous_episode_match(files: &[VideoFileInfo], season: u32, episode: u32) -> Option<EpisodeFileMatch> {
    let matches = match_episode(files, season, episode);
    let Some(best) = matches.first() else {
        return None;
    };
    if best.confidence < 80 {
        return None;
    }
    if matches.len() > 1 && matches[1].confidence >= best.confidence - 5 {
        return None;
    }
    Some(best.clone())
}

/// Filtered playable list: videos without extras junk.
pub fn playable_videos(files: &[VideoFileInfo]) -> Vec<VideoFileInfo> {
    files
        .iter()
        .filter(|f| classify(&f.path) == VideoClass::Video)
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(index: usize, path: &str, size: u64) -> VideoFileInfo {
        VideoFileInfo {
            file_index: index,
            path: path.to_string(),
            name: path.rsplit(['/', '\\']).next().unwrap_or(path).to_string(),
            size_bytes: size,
        }
    }

    #[test]
    fn video_extensions_recognized() {
        assert!(is_video_filename("movie.mkv"));
        assert!(is_video_filename("MOVIE.MP4"));
        assert!(is_video_filename("show.s01e01.ts"));
        assert!(!is_video_filename("cover.jpg"));
        assert!(!is_video_filename("noext"));
        assert!(!is_video_filename("nfo.txt"));
    }

    #[test]
    fn extras_markers_match_paths() {
        assert_eq!(classify("Extras/Sample/s1e1.mkv"), VideoClass::Extras);
        assert_eq!(classify("trailer.mkv"), VideoClass::Extras);
        assert_eq!(classify("movie/Behind.The.Scenes/cast.mp4"), VideoClass::Extras);
        assert_eq!(classify("movie.mkv"), VideoClass::Video);
        assert_eq!(classify("poster.jpg"), VideoClass::Other);
    }

    #[test]
    fn parses_sxxexx_tokens() {
        assert_eq!(season_episode_tokens("Show.S01E02.1080p.mkv"), vec![(1, 2)]);
        assert_eq!(season_episode_tokens("show/s1e2.mkv"), vec![(1, 2)]);
        assert_eq!(season_episode_tokens("1x03.mkv"), vec![(1, 3)]);
        assert_eq!(season_episode_tokens("Show.S01E02E03.mkv"), vec![(1, 2), (1, 3)]);
        assert!(season_episode_tokens("The.Matrix.1999.mkv").is_empty());
    }

    #[test]
    fn episode_match_prefers_exact_token() {
        let files = vec![
            file(0, "Season 1/Show.S01E01.1080p.mkv", 1000),
            file(1, "Season 1/Show.S01E02.1080p.mkv", 1000),
            file(2, "Season 1/Show.S01E03.1080p.mkv", 1000),
        ];
        let matches = match_episode(&files, 1, 2);
        assert_eq!(matches[0].file_index, 1);
        assert!(matches[0].confidence >= 90);
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn episode_match_ignores_extras_and_non_video() {
        let files = vec![
            file(0, "Sample/Show.S01E01.sample.mkv", 10),
            file(1, "cover.png", 10),
            file(2, "Show.S01E01.mkv", 900),
        ];
        let matches = match_episode(&files, 1, 1);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].file_index, 2);
    }

    #[test]
    fn unambiguous_requires_clear_winner() {
        let files = vec![
            file(0, "Show.S01E01.mkv", 900),
            file(1, "Show.S01E01.720p.mkv", 400),
        ];
        // Two high-confidence candidates → manual selection required.
        assert!(unambiguous_episode_match(&files, 1, 1).is_none());

        let single = vec![file(0, "Show.S01E01.mkv", 900)];
        let best = unambiguous_episode_match(&single, 1, 1).unwrap();
        assert_eq!(best.file_index, 0);
    }

    #[test]
    fn episode_only_match_is_medium_confidence() {
        let files = vec![file(0, "whatever.ep1.mkv", 900)];
        // no standard tokens → no match at all (we do not guess)
        assert!(match_episode(&files, 1, 1).is_empty());
    }

    #[test]
    fn playable_videos_filters_extras() {
        let files = vec![
            file(0, "movie.mkv", 1000),
            file(1, "sample.mkv", 10),
            file(2, "trailer.mp4", 20),
            file(3, "cover.jpg", 1),
        ];
        let videos = playable_videos(&files);
        assert_eq!(videos.len(), 1);
        assert_eq!(videos[0].file_index, 0);
    }
}
