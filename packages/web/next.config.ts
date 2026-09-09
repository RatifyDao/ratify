import type { NextConfig } from "next";

const config: NextConfig = {
  // The index is read through better-sqlite3, which is a native module and
  // must not be bundled into the server build.
  serverExternalPackages: ["better-sqlite3"],
};

export default config;
