// A bounded, on-demand WebGL material for the catalog ring. HTML owns all meaning.
const vertex = `
attribute vec2 position;
varying vec2 point;
void main() { point = position; gl_Position = vec4(position, 0.0, 1.0); }
`;
const fragment = `
precision mediump float;
varying vec2 point;
uniform vec4 ends;
uniform vec3 colors[4];
uniform float empty;
uniform float segments;
void main() {
  float radius = length(point);
  float crossSection = (radius - 0.77) / 0.15;
  float coverage = 1.0 - smoothstep(0.97, 1.0, abs(crossSection));
  if (coverage <= 0.0) discard;
  float angle = fract(atan(point.x, point.y) / 6.2831853 + 1.0);
  if (segments > 1.0) {
    float slot = fract(angle * segments);
    float edge = min(slot, 1.0 - slot) / segments;
    coverage *= smoothstep(0.001, 0.004, edge);
  }
  float boundary = min(min(abs(angle - ends.x), abs(angle - ends.y)), abs(angle - ends.z));
  if (empty < 0.5) coverage *= smoothstep(0.001, 0.003, boundary);
  vec3 color = colors[0];
  if (angle >= ends.x) color = colors[1];
  if (angle >= ends.y) color = colors[2];
  if (angle >= ends.z) color = colors[3];
  if (empty > 0.5) color = colors[2];
  vec2 radial = point / max(radius, 0.01);
  vec3 normal = normalize(vec3(radial * crossSection, sqrt(max(0.0, 1.0 - crossSection * crossSection))));
  vec3 light = normalize(vec3(-0.45, 0.6, 1.4));
  float diffuse = max(dot(normal, light), 0.0);
  float specular = pow(max(dot(reflect(-light, normal), vec3(0.0, 0.0, 1.0)), 0.0), 30.0);
  float bevel = smoothstep(0.55, 0.9, abs(crossSection));
  float engraving = 0.985 + 0.015 * cos(angle * 1256.637);
  color = color * (0.82 + diffuse * 0.18) * engraving + vec3(specular * 0.065 + bevel * 0.035);
  gl_FragColor = vec4(color, coverage);
}
`;
function rgb(value) {
  const hex = value.trim();
  if (/^#[0-9a-f]{6}$/i.test(hex)) return [1, 3, 5].map(i => parseInt(hex.slice(i, i + 2), 16) / 255);
  const channels = hex.match(/[\d.]+/g);
  return channels?.length >= 3 ? channels.slice(0, 3).map(Number).map(v => v / 255) : [0.5, 0.5, 0.5];
}
export class ActivityMaterial {
  constructor(root) {
    this.root = root; this.document = root.ownerDocument; this.disposed = false; this.counts = null;
    this.canvas = this.document.createElement("canvas");
    this.canvas.className = "activity-material";
    this.canvas.setAttribute("aria-hidden", "true");
    root.prepend(this.canvas);
    this.scheme = matchMedia("(prefers-color-scheme: light)");
    this.contrast = matchMedia("(forced-colors: active)");
    this.refresh = () => this.draw();
    this.lost = event => {
      event.preventDefault(); this.program = null; this.buffer = null;
      delete root.dataset.material;
    };
    this.restored = () => { this.initialize(); this.draw(); };
    this.canvas.addEventListener("webglcontextlost", this.lost);
    this.canvas.addEventListener("webglcontextrestored", this.restored);
    this.resize = new ResizeObserver(this.refresh); this.resize.observe(root);
    this.theme = new MutationObserver(this.refresh);
    this.theme.observe(this.document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
    this.scheme.addEventListener("change", this.refresh); this.contrast.addEventListener("change", this.refresh);
    this.document.addEventListener("visibilitychange", this.refresh);
    this.onPageHide = event => { if (!event.persisted) this.dispose(); };
    this.document.defaultView.addEventListener("pagehide", this.onPageHide);
    this.initialize();
  }
  initialize() {
    if (this.disposed) return;
    const gl = this.gl ?? this.canvas.getContext("webgl", { alpha: true, antialias: true, depth: false, stencil: false, powerPreference: "low-power", preserveDrawingBuffer: false });
    if (!gl || gl.isContextLost()) return;
    this.gl = gl;
    const shaders = [];
    let program = null, buffer = null;
    try {
      for (const [type, source] of [[gl.VERTEX_SHADER, vertex], [gl.FRAGMENT_SHADER, fragment]]) {
        const shader = gl.createShader(type);
        if (!shader) throw Error("Shader unavailable");
        shaders.push(shader); gl.shaderSource(shader, source); gl.compileShader(shader);
        if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) throw Error("Shader compilation failed");
      }
      program = gl.createProgram();
      if (!program) throw Error("Program unavailable");
      for (const shader of shaders) gl.attachShader(program, shader);
      gl.linkProgram(program);
      if (!gl.getProgramParameter(program, gl.LINK_STATUS)) throw Error("Shader link failed");
      buffer = gl.createBuffer();
      if (!buffer) throw Error("Buffer unavailable");
      gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
      gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1,-1,1,-1,-1,1,-1,1,1,-1,1,1]), gl.STATIC_DRAW);
      this.program = program; this.buffer = buffer;
    } catch {
      if (program) gl.deleteProgram(program);
      if (buffer) gl.deleteBuffer(buffer);
      this.program = null; this.buffer = null; delete this.root.dataset.material;
    } finally { for (const shader of shaders) gl.deleteShader(shader); }
  }
  update(counts) { this.counts = counts; this.draw(); }
  draw() {
    const gl = this.gl;
    if (this.disposed || !gl || !this.program || !this.counts || gl.isContextLost()) return;
    if (this.contrast.matches) { delete this.root.dataset.material; return; }
    if (this.document.hidden || !this.root.clientWidth || !this.root.clientHeight) return;
    const size = Math.min(384, Math.round(this.root.clientWidth * Math.min(devicePixelRatio || 1, 2)));
    if (!size) return;
    if (this.canvas.width !== size) this.canvas.width = this.canvas.height = size;
    gl.viewport(0, 0, size, size); gl.clearColor(0, 0, 0, 0); gl.clear(gl.COLOR_BUFFER_BIT);
    gl.useProgram(this.program); gl.bindBuffer(gl.ARRAY_BUFFER, this.buffer);
    const position = gl.getAttribLocation(this.program, "position");
    gl.enableVertexAttribArray(position); gl.vertexAttribPointer(position, 2, gl.FLOAT, false, 0, 0);
    const total = this.counts.reduce((sum, count) => sum + count, 0);
    let sum = 0;
    const ends = this.counts.map(count => { sum += count; return total ? sum / total : 1; });
    const style = getComputedStyle(this.root);
    const colors = ["success", "warning", "muted", "danger"].flatMap(name => rgb(style.getPropertyValue("--" + name)));
    gl.uniform4fv(gl.getUniformLocation(this.program, "ends"), ends);
    gl.uniform3fv(gl.getUniformLocation(this.program, "colors[0]"), colors);
    gl.uniform1f(gl.getUniformLocation(this.program, "empty"), total ? 0 : 1);
    gl.uniform1f(gl.getUniformLocation(this.program, "segments"), total <= 64 ? total : 0);
    gl.drawArrays(gl.TRIANGLES, 0, 6); gl.flush();
    this.root.dataset.material = "rendered";
  }
  dispose() {
    if (this.disposed) return;
    this.disposed = true; this.resize.disconnect(); this.theme.disconnect();
    this.scheme.removeEventListener("change", this.refresh); this.contrast.removeEventListener("change", this.refresh);
    this.document.removeEventListener("visibilitychange", this.refresh);
    this.document.defaultView.removeEventListener("pagehide", this.onPageHide);
    this.canvas.removeEventListener("webglcontextlost", this.lost); this.canvas.removeEventListener("webglcontextrestored", this.restored);
    if (this.program) this.gl.deleteProgram(this.program);
    if (this.buffer) this.gl.deleteBuffer(this.buffer);
    this.gl?.getExtension("WEBGL_lose_context")?.loseContext();
    this.canvas.remove(); delete this.root.dataset.material;
  }
}
