import "./globals.css";

export const metadata = {
  title: "ISMV",
  description: "Proyecto Next.js",
};

export default function RootLayout({ children }) {
  return (
    <html lang="es">
      <body>{children}</body>
    </html>
  );
}
