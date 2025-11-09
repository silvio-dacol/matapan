import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  async rewrites() {
    return [
      {
        source: "/api/:path*",
        // Proxy all /api/* requests from the frontend dev server to the backend API
        // NOTE: The previous destination missed the /api prefix, causing 404s like /dashboard
        destination: "http://localhost:3000/api/:path*",
      },
    ];
  },
};

export default nextConfig;
