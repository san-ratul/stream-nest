import { reactive } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openInPlayer } from "../stores/player";
import { streamUrl } from "../stores/torrents";

/** Stages: null | connecting | metadata | files | starting. Never fake progress. */
export const playbackStore = reactive({
  stage: null,
  error: null,
  prepared: null,
});

let stageUnlisten = null;

export async function listenPlaybackStages() {
  if (stageUnlisten) stageUnlisten();
  stageUnlisten = await listen("playback:stage", (e) => {
    playbackStore.stage = e.payload;
  });
}

export function friendlyError(err) {
  const msg = String(err?.message ?? err);
  if (msg.includes("engine is not ready")) return "The torrent engine is still starting. Try again in a moment.";
  if (msg.includes("Could not retrieve source metadata")) {
    if (msg.includes("No peers")) return "No peers are currently available for this source.";
    return "Could not retrieve source metadata.";
  }
  if (msg.includes("No playable video file")) return "No playable video file found in this source.";
  if (msg.includes("could not be added")) return "This source is invalid.";
  if (msg.includes("invalid peer address")) return "This source is invalid: bad peer address.";
  if (msg.includes("cannot read test torrent")) return "Dev fixture torrent file is missing.";
  return msg.length > 220 ? "Unable to start playback." : msg;
}

async function prepare(source) {
  playbackStore.error = null;
  playbackStore.prepared = null;
  playbackStore.stage = "connecting";
  let prepared;
  if (source.magnet_uri) {
    prepared = await invoke("playback_prepare_uri", {
      uri: source.magnet_uri,
      peers: source.initial_peers?.length ? source.initial_peers : null,
    });
  } else if (source.torrent_file) {
    const bytes = await invoke("dev_read_test_torrent", {
      name: source.torrent_file.split(/[\\/]/).pop(),
    });
    prepared = await invoke("playback_prepare_file", {
      contentsBase64: bytes,
      peers: source.initial_peers?.length ? source.initial_peers : null,
    });
  } else {
    throw new Error("This source cannot be streamed in-app");
  }
  playbackStore.prepared = prepared;
  playbackStore.stage = "files";
  return prepared;
}

/**
 * Prepares the source and returns what to do next:
 *   { started }                                  playback handed to the player
 *   { needsFileChoice, prepared, videos }        movie with multiple files
 *   { needsEpisodeChoice, prepared, matches }    ambiguous episode match
 * throws friendly Error on failure.
 */
export async function startAuthorizedPlayback(options) {
  const { source, tmdbKey, title, season, episode, onNeedFileChoice } = options;
  try {
    const prepared = await prepare(source);
    const videos = prepared.videos ?? [];
    if (!videos.length) {
      throw new Error("No playable video file found in this source");
    }

    let fileId = null;
    let fileTitle = null;

    if (season != null && episode != null) {
      const matches = await invoke("episode_matches", {
        infoHash: prepared.info_hash,
        season,
        episode,
      }).catch(() => []);
      const top = matches[0];
      const clearWinner =
        top &&
        top.confidence >= 80 &&
        (matches.length === 1 || top.confidence - matches[1].confidence >= 5);
      if (clearWinner) {
        fileId = top.file_index;
        fileTitle = top.name;
      } else if (matches.length > 0) {
        playbackStore.stage = null;
        return { needsEpisodeChoice: true, prepared, matches, source, tmdbKey, title };
      }
      // No token match at all: fall through to the generic file choice over
      // playable videos (manual selection, never a silent guess).
    }

    if (fileId == null && videos.length === 1) {
      fileId = videos[0].file_index;
      fileTitle = videos[0].name;
    }

    if (fileId == null) {
      if (!onNeedFileChoice) {
        playbackStore.stage = null;
        return { needsFileChoice: true, prepared, videos, source, tmdbKey, title };
      }
      fileId = await onNeedFileChoice(videos);
      if (fileId == null) {
        playbackStore.stage = null;
        return { cancelled: true };
      }
      fileTitle = videos.find((v) => v.file_index === fileId)?.name ?? title;
    }

    return await streamPrepared(prepared, { source, tmdbKey, title, fileId, fileTitle });
  } catch (err) {
    playbackStore.stage = null;
    const friendly = friendlyError(err);
    playbackStore.error = friendly;
    throw new Error(friendly);
  }
}

/** Completes a pending file/episode choice. */
export async function streamPrepared(prepared, { source, tmdbKey, title, fileId, fileTitle }) {
  playbackStore.stage = "starting";
  try {
    await invoke("playback_begin", {
      mediaKey: tmdbKey ?? `${prepared.info_hash}:${fileId}`,
      infoHash: prepared.info_hash,
      fileId,
      sourceId: source.provider_id,
    });
    const url = await streamUrl(prepared.info_hash, fileId);
    openInPlayer({
      src: url,
      title: fileTitle ?? title ?? prepared.torrent_name,
      torrentId: prepared.info_hash,
      fileId,
      infoHash: prepared.info_hash,
      tmdbKey: tmdbKey ?? null,
    });
    playbackStore.stage = null;
    return { started: true, infoHash: prepared.info_hash, fileId };
  } catch (err) {
    playbackStore.stage = null;
    const friendly = friendlyError(err);
    playbackStore.error = friendly;
    throw new Error(friendly);
  }
}

export function endPlaybackSession(mediaKey) {
  if (mediaKey) {
    invoke("playback_end", { mediaKey }).catch(() => {});
  }
}

/**
 * Streams a persisted source record (from the Source Library or details pages).
 * The record's stored input (magnet/ torrent copy) is resolved inside Rust.
 */
export async function startRecordPlayback(record, { onNeedFileChoice, title } = {}) {
  playbackStore.error = null;
  playbackStore.stage = "connecting";
  try {
    const prepared = await invoke("playback_prepare_record", { sourceId: record.id });
    playbackStore.prepared = prepared;
    playbackStore.stage = "files";
    const videos = prepared.videos ?? [];
    if (!videos.length) throw new Error("No playable video file found in this source");

    let fileId = null;
    let fileTitle = null;

    // Episode association on the record → metadata match first.
    const isEpisode = record.tmdb_media_type === "tv" && record.season_number != null && record.episode_number != null;
    if (isEpisode) {
      const matches = await invoke("episode_matches", {
        infoHash: prepared.info_hash,
        season: record.season_number,
        episode: record.episode_number,
      }).catch(() => []);
      const top = matches[0];
      const clearWinner =
        top && top.confidence >= 80 && (matches.length === 1 || top.confidence - matches[1].confidence >= 5);
      if (clearWinner) {
        fileId = top.file_index;
        fileTitle = top.name;
      }
    }

    if (fileId == null && videos.length === 1 && !isEpisode) {
      fileId = videos[0].file_index;
      fileTitle = videos[0].name;
    }

    if (fileId == null) {
      if (!onNeedFileChoice) {
        playbackStore.stage = null;
        return { needsFileChoice: true, prepared, videos, record };
      }
      fileId = await onNeedFileChoice(videos);
      if (fileId == null) {
        playbackStore.stage = null;
        return { cancelled: true };
      }
      fileTitle = videos.find((v) => v.file_index === fileId)?.name ?? title;
    }

    playbackStore.stage = "starting";
    const tmdbKey =
      record.tmdb_media_type === "movie" && record.tmdb_id
        ? `tmdb:movie:${record.tmdb_id}`
        : record.tmdb_media_type === "tv" && record.tmdb_id
          ? record.season_number != null && record.episode_number != null
            ? `tmdb:tv:${record.tmdb_id}:${record.season_number}:${record.episode_number}`
            : `tmdb:tv:${record.tmdb_id}`
          : null;

    await invoke("playback_begin", {
      mediaKey: tmdbKey ?? `${prepared.info_hash}:${fileId}`,
      infoHash: prepared.info_hash,
      fileId,
      sourceId: record.provider_key ?? `source:${record.id}`,
    });
    const url = await streamUrl(prepared.info_hash, fileId);
    openInPlayer({
      src: url,
      title: fileTitle ?? title ?? record.display_name,
      torrentId: prepared.info_hash,
      fileId,
      infoHash: prepared.info_hash,
      tmdbKey,
    });
    playbackStore.stage = null;
    return { started: true };
  } catch (err) {
    playbackStore.stage = null;
    const friendly = friendlyError(err);
    playbackStore.error = friendly;
    throw new Error(friendly);
  }
}
