import type { Metadata, Viewport } from "next";
import type { ReactNode } from "react";

import "./globals.css";

export const metadata: Metadata = {
  title: "Service-Arb panel",
  // Internal, behind sign-in: nothing here is for a search engine.
  robots: { index: false, follow: false },
};

export const viewport: Viewport = {
  width: "device-width",
  initialScale: 1,
  // The tab bar pads itself by env(safe-area-inset-bottom), which needs the page under the notch.
  viewportFit: "cover",
};

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="en">
      <body className="min-h-svh bg-background font-sans text-ink antialiased">{children}</body>
    </html>
  );
}
