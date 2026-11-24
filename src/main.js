import { createApp } from "vue";
import App from "./App.vue";
import { router } from "./router";
import { startTorrentStore } from "./stores/torrents";
import { hydrateSettings } from "./stores/settings";
import "./style.css";

Promise.all([startTorrentStore(), hydrateSettings()]).finally(() => {
  createApp(App).use(router).mount("#app");
});
