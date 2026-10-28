// Draws live view frames with WebGL2, in one pass on the GPU:
//   camera picture -> (with a look) conversion -> the look's LUT, then zebra.
//
// The conversion turns the camera's live view (Picture Style Neutral) into the
// input Argentum gives a LUT on its "Linear" rendering, so LUTs made in
// Argentum apply as they do there (see src/luts/r6iii-neutral-to-linear.cube).
//
// Zebra stripes mark pixels where any colour channel of the camera picture
// reaches the threshold, so one blown channel (a clipped red sky) shows as well
// as overall white, and a look never hides clipping. The stripe pattern follows
// open-eos-control (Apache-2.0, bridge/open_eos_bridge/static/monitoring.js at
// 089e71a3): light/dark diagonal bands; theirs tests luminance only.

import { parseCube, type Cube } from "./cube";
import conversionText from "./luts/r6iii-neutral-to-linear.cube?raw";

const VERTEX = `#version 300 es
in vec2 corner;
out vec2 uv;
void main() {
  uv = vec2(corner.x * 0.5 + 0.5, 0.5 - corner.y * 0.5);
  gl_Position = vec4(corner, 0.0, 1.0);
}`;

const FRAGMENT = `#version 300 es
precision highp float;
precision highp sampler3D;
in vec2 uv;
uniform sampler2D frame;
uniform sampler3D conversion;
uniform sampler3D look;
uniform float conversionSize;
uniform float lookSize;       // 0 = no look
uniform vec3 lookMin;
uniform vec3 lookScale;       // 1 / (domain max - domain min)
uniform float zebraLevel;     // 0..1; above 1 turns zebra off
uniform float stripeShift;    // moves the stripes so they never look like texture
out vec4 color;

vec3 sampleLut(sampler3D lut, vec3 c, float size) {
  return texture(lut, (clamp(c, 0.0, 1.0) * (size - 1.0) + 0.5) / size).rgb;
}

void main() {
  vec3 camera = texture(frame, uv).rgb;
  vec3 rgb = camera;
  if (lookSize > 0.0) {
    vec3 linear = sampleLut(conversion, camera, conversionSize);
    rgb = sampleLut(look, (linear - lookMin) * lookScale, lookSize);
  }
  if (max(camera.r, max(camera.g, camera.b)) >= zebraLevel) {
    float band = mod((gl_FragCoord.x + gl_FragCoord.y + stripeShift) / 6.0, 2.0);
    rgb = band < 1.0 ? mix(rgb, vec3(1.0), 0.7) : mix(rgb, vec3(0.0), 0.5);
  }
  color = vec4(rgb, 1.0);
}`;

export type Renderer = {
  /** `look` is drawn when given; changing it only re-uploads when it is a different LUT. */
  draw(image: ImageBitmap, zebraPercent: number | null, look: Cube | null): void;
};

/** Parsed once: every renderer shares it. */
let conversionCube: Cube | null = null;

export function createRenderer(canvas: HTMLCanvasElement): Renderer {
  const gl = canvas.getContext("webgl2");
  if (!gl) throw new Error("WebGL2 is not available");
  const program = link(gl, VERTEX, FRAGMENT);
  gl.useProgram(program);
  const uniform = (name: string) => gl.getUniformLocation(program, name);

  // Two triangles covering the canvas.
  gl.bindBuffer(gl.ARRAY_BUFFER, gl.createBuffer());
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]), gl.STATIC_DRAW);
  const corner = gl.getAttribLocation(program, "corner");
  gl.enableVertexAttribArray(corner);
  gl.vertexAttribPointer(corner, 2, gl.FLOAT, false, 0, 0);

  const frameTexture = texture(gl, gl.TEXTURE0, gl.TEXTURE_2D);
  gl.uniform1i(uniform("frame"), 0);

  conversionCube ??= parseCube(conversionText, "conversion");
  texture(gl, gl.TEXTURE1, gl.TEXTURE_3D);
  upload3d(gl, conversionCube);
  gl.uniform1i(uniform("conversion"), 1);
  gl.uniform1f(uniform("conversionSize"), conversionCube.size);

  const lookTexture = texture(gl, gl.TEXTURE2, gl.TEXTURE_3D);
  gl.uniform1i(uniform("look"), 2);
  let shownLook: Cube | null = null;

  const zebraLevel = uniform("zebraLevel");
  const stripeShift = uniform("stripeShift");
  const lookSize = uniform("lookSize");

  return {
    draw(image, zebraPercent, look) {
      if (canvas.width !== image.width || canvas.height !== image.height) {
        canvas.width = image.width;
        canvas.height = image.height;
      }
      if (look !== shownLook) {
        shownLook = look;
        if (look) {
          gl.activeTexture(gl.TEXTURE2);
          gl.bindTexture(gl.TEXTURE_3D, lookTexture);
          upload3d(gl, look);
          gl.uniform3fv(uniform("lookMin"), look.domainMin);
          gl.uniform3fv(uniform("lookScale"), look.domainMax.map((max, i) => 1 / (max - look.domainMin[i])));
        }
        gl.uniform1f(lookSize, look ? look.size : 0);
      }
      gl.viewport(0, 0, canvas.width, canvas.height);
      gl.activeTexture(gl.TEXTURE0);
      gl.bindTexture(gl.TEXTURE_2D, frameTexture);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, image);
      gl.uniform1f(zebraLevel, zebraPercent === null ? 2 : zebraPercent / 100);
      gl.uniform1f(stripeShift, (performance.now() / 40) % 12);
      gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
    },
  };
}

/** A linearly filtered, edge-clamped texture, bound on `unit`. */
function texture(gl: WebGL2RenderingContext, unit: number, target: number): WebGLTexture {
  const t = gl.createTexture();
  gl.activeTexture(unit);
  gl.bindTexture(target, t);
  gl.texParameteri(target, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
  gl.texParameteri(target, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
  for (const wrap of [gl.TEXTURE_WRAP_S, gl.TEXTURE_WRAP_T, gl.TEXTURE_WRAP_R]) {
    gl.texParameteri(target, wrap, gl.CLAMP_TO_EDGE);
  }
  return t;
}

/** .cube rows list red fastest, which is the 3D texture's x. */
function upload3d(gl: WebGL2RenderingContext, cube: Cube) {
  const n = cube.size;
  gl.texImage3D(gl.TEXTURE_3D, 0, gl.RGB16F, n, n, n, 0, gl.RGB, gl.FLOAT, cube.values);
}

function link(gl: WebGL2RenderingContext, vertex: string, fragment: string): WebGLProgram {
  const program = gl.createProgram();
  for (const [type, source] of [
    [gl.VERTEX_SHADER, vertex],
    [gl.FRAGMENT_SHADER, fragment],
  ] as const) {
    const shader = gl.createShader(type)!;
    gl.shaderSource(shader, source);
    gl.compileShader(shader);
    if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(shader) ?? "shader");
    gl.attachShader(program, shader);
  }
  gl.linkProgram(program);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(program) ?? "link");
  return program;
}
