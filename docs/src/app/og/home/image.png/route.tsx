import { createHomeImageResponse } from '../home-image';

export const revalidate = false;

export async function GET() {
  return createHomeImageResponse();
}
