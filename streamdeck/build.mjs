// Bundles src/plugin.ts into the plugin's bin/plugin.js (the plugin folder is shipped without node_modules).
import { build, context } from "esbuild";

const options = {
  entryPoints: ["src/plugin.ts"],
  outfile: "io.github.chrispygee.roomtone.sdPlugin/bin/plugin.js",
  bundle: true,
  platform: "node",
  target: "node24",
  format: "esm",
  sourcemap: false,
  minify: false,
  banner: { js: "import { createRequire } from 'module'; const require = createRequire(import.meta.url);" },
};

if (process.argv.includes("--watch")) {
  const ctx = await context(options);
  await ctx.watch();
  console.log("Watching src/ ...");
} else {
  await build(options);
  console.log("Built io.github.chrispygee.roomtone.sdPlugin/bin/plugin.js");
}
