import { createApp } from "vue";
import { router } from "./router";
import App from "./App.vue";
import { startTorrentStore } from "./stores/torrents";
import { hydrateSettings } from "./stores/settings";
import { restorePlayer } from "./stores/player";
import "./style.css";

document.addEventListener("contextmenu", (e) => e.preventDefault());

restorePlayer();
Promise.all([startTorrentStore(), hydrateSettings()]).finally(() => {
  createApp(App).use(router).mount("#app");
});
