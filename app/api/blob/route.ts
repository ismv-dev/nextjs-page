import { list, head, get } from '@vercel/blob';
import { NextResponse } from 'next/server';

async function getBlobs(path) {
  const { blobs } = await list();
  
  return blobs
    .filter(blob => blob.pathname.startsWith(`${path}/`))
    .map(blob => ({
      url: blob.url,
      pathname: blob.pathname,
      size: blob.size,
      uploadedAt: blob.uploadedAt
    }));
}

export async function GET(request: Request) {
  // 1. AQUI: Implementa tu lógica de autenticación (ej: getAuth(request))
  // if (!usuarioAutenticado) return new Response("No autorizado", { status: 401 });

  const { searchParams } = new URL(request.url);
  const blobUrl = searchParams.get('url');

  if (!blobUrl) return new Response("Falta URL", { status: 400 });

  try {
    // 2. Obtener el blob usando el token privado
    const blob = await get(blobUrl); 
    
    if (!blob) return new Response("No encontrado", { status: 404 });

    // 3. Devolver el archivo con las cabeceras correctas para PDFs
    return new NextResponse(blob, {
      headers: {
        'Content-Type': 'application/pdf',
        'Content-Disposition': `inline; filename="documento.pdf"`,
      },
    });
  } catch (error) {
    return new Response("Error al obtener el archivo", { status: 500 });
  }
}
