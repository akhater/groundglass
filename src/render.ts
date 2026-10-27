// Draws live view frames with WebGL2. Zebra runs here on the GPU; LUTs will join it.
//
// Zebra stripes mark pixels where any colour channel reaches the threshold, so one
// blown channel (a clipped red sky) shows as well as overall white. The stripe
// pattern follows open-eos-control (Apache-2.0, bridge/open_eos_bridge/static/monitoring.js
// at 089e71a3): light/dark diagonal bands; theirs tests luminance only.

const VERTEX = `#version 300 es
in vec2 corner;
out vec2 uv;
void main() {
  uv = vec2(corner.x * 0.5 + 0.5, 0.5 - corner.y * 0.5);
  gl_Position = vec4(corner, 0.0, 1.0);
}`;

const FRAGMENT = `#version 300 es
precision mediump float;
in vec2 uv;
uniform sampler2D frame;
uniform float zebraLevel;  // 0..1; above 1 turns zebra off
uniform float stripeShift; // moves the stripes so they never look like texture
out vec4 color;
void main() {
  vec3 rgb = texture(frame, uv).rgb;
  if (max(rgb.r, max(rgb.g, rgb.b)) >= zebraLevel) {
    float band = mod((gl_FragCoord.x + gl_FragCoord.y + stripeShift) / 6.0, 2.0);
    rgb = band < 1.0 ? mix(rgb, vec3(1.0), 0.7) : mix(rgb, vec3(0.0), 0.5);
  }
  color = vec4(rgb, 1.0);
}`;

export type Renderer = { draw(image: ImageBitmap, zebraPercent: number | null): void };

export function createRenderer(canvas: HTMLCanvasElement): Renderer {
  const gl = canvas.getContext("webgl2");
  if (!gl) throw new Error("WebGL2 is not available");
  const program = link(gl, VERTEX, FRAGMENT);
  gl.useProgram(program);

  // Two triangles covering the canvas.
  gl.bindBuffer(gl.ARRAY_BUFFER, gl.createBuffer());
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]), gl.STATIC_DRAW);
  const corner = gl.getAttribLocation(program, "corner");
  gl.enableVertexAttribArray(corner);
  gl.vertexAttribPointer(corner, 2, gl.FLOAT, false, 0, 0);

  gl.bindTexture(gl.TEXTURE_2D, gl.createTexture());
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);

  const zebraLevel = gl.getUniformLocation(program, "zebraLevel");
  const stripeShift = gl.getUniformLocation(program, "stripeShift");

  return {
    draw(image, zebraPercent) {
      if (canvas.width !== image.width || canvas.height !== image.height) {
        canvas.width = image.width;
        canvas.height = image.height;
      }
      gl.viewport(0, 0, canvas.width, canvas.height);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, image);
      gl.uniform1f(zebraLevel, zebraPercent === null ? 2 : zebraPercent / 100);
      gl.uniform1f(stripeShift, (performance.now() / 40) % 12);
      gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
    },
  };
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
