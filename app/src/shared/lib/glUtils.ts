let cachedSoftwareRaster: boolean | null = null;

/**
 * Detects whether the current WebGL context is backed by a CPU-only software rasterizer
 * (llvmpipe, SwiftShader, softpipe, etc.). Cached at module level so the probe
 * only runs once per app lifecycle.
 */
export function isSoftwareRasterizer(): boolean {
  if (cachedSoftwareRaster !== null) return cachedSoftwareRaster;
  if (typeof document === "undefined") return false;
  try {
    const gl = document.createElement("canvas").getContext("webgl");
    if (!gl) {
      cachedSoftwareRaster = false;
      return false;
    }
    const ext = gl.getExtension("WEBGL_debug_renderer_info");
    const rendererStr = ext
      ? String(gl.getParameter(ext.UNMASKED_RENDERER_WEBGL))
      : "";
    gl.getExtension("WEBGL_lose_context")?.loseContext();
    cachedSoftwareRaster = /llvmpipe|softpipe|swiftshader|software|basic render/i.test(rendererStr);
    return cachedSoftwareRaster;
  } catch {
    cachedSoftwareRaster = false;
    return false;
  }
}
