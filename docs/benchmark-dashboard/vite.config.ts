import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "node:path";

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./"),
    },
  },
  define: {
    "process.env.NODE_ENV": JSON.stringify("production"),
  },
  build: {
    lib: {
      entry: "./embed.tsx",
      name: "MyApp",
      fileName: "benchmark-dashboard",
      formats: ["iife"],
    },
    outDir: "dist",
  },
});
