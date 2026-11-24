import { createRouter, createWebHashHistory } from "vue-router";
import HomePage from "../pages/HomePage.vue";
import DiscoverPage from "../pages/DiscoverPage.vue";
import AddTorrentPage from "../pages/AddTorrentPage.vue";
import TorrentDetailsPage from "../pages/TorrentDetailsPage.vue";
import PlayerPage from "../pages/PlayerPage.vue";
import DownloadManagerPage from "../pages/DownloadManagerPage.vue";
import LibraryPage from "../pages/LibraryPage.vue";
import FavoritesPage from "../pages/FavoritesPage.vue";
import SettingsPage from "../pages/SettingsPage.vue";
import AboutPage from "../pages/AboutPage.vue";
import DevHarnessPage from "../pages/DevHarnessPage.vue";

export const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: "/", name: "home", component: HomePage },
    { path: "/discover", name: "discover", component: DiscoverPage },
    { path: "/add", name: "add", component: AddTorrentPage },
    { path: "/torrent/:id", name: "torrent-details", component: TorrentDetailsPage },
    { path: "/player", name: "player", component: PlayerPage },
    { path: "/downloads", name: "downloads", component: DownloadManagerPage },
    { path: "/library", name: "library", component: LibraryPage },
    { path: "/favorites", name: "favorites", component: FavoritesPage },
    { path: "/settings", name: "settings", component: SettingsPage },
    { path: "/about", name: "about", component: AboutPage },
    { path: "/dev", name: "dev", component: DevHarnessPage },
  ],
});
