import { VitePWA } from "vite-plugin-pwa";

export function serverPwaPlugin() {
  return VitePWA({
    injectRegister: false,
    registerType: "prompt",
    includeAssets: ["favicon.svg", "apple-touch-icon.png"],
    manifest: {
      id: "/",
      name: "Lapis Server",
      short_name: "Lapis Server",
      start_url: "/",
      scope: "/",
      display: "standalone",
      display_override: ["window-controls-overlay", "standalone"],
      background_color: "#f6f6f6",
      theme_color: "#f6f6f6",
      icons: [
        { src: "pwa-192x192.png", sizes: "192x192", type: "image/png" },
        { src: "pwa-512x512.png", sizes: "512x512", type: "image/png" },
        {
          src: "pwa-512x512-maskable.png",
          sizes: "512x512",
          type: "image/png",
          purpose: "maskable",
        },
      ],
    },
    workbox: {
      cacheId: "lapis-server",
      cleanupOutdatedCaches: true,
      clientsClaim: true,
      skipWaiting: false,
      navigateFallback: "index.html",
      navigateFallbackAllowlist: [/^\/(?:notes\/[^?]*)?$/],
      navigateFallbackDenylist: [/[?&](?:sync-connect|mcp-consent)=/],
      globPatterns: [
        "index.html",
        "assets/**/*",
        "plugin-host-*.js",
        "pwa-*.png",
        "manifest.webmanifest",
      ],
      maximumFileSizeToCacheInBytes: 50 * 1024 * 1024,
    },
  });
}
