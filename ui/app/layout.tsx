import type { Metadata } from "next";
import "../styles/theme.css";
import "../styles/app.css";
export const metadata: Metadata = { title: "Motetto", description: "Local meeting notes" };
export default function Layout({children}:Readonly<{children:React.ReactNode}>) {
  return <html lang="en"><body>{children}</body></html>;
}
