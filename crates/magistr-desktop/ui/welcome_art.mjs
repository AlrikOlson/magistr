// A decorative, bounded welcome illustration. No session data or interaction lives here.
const vertex = `
attribute vec2 position;
void main() { gl_Position = vec4(position, 0.0, 1.0); }
`;
const fragment = `
precision mediump float;
uniform vec2 resolution;
uniform float time;
uniform float light;
void main() {
  vec2 p = (gl_FragCoord.xy * 2.0 - resolution) / min(resolution.x, resolution.y);
  p = mat2(0.9, -0.43, 0.43, 0.9) * p;
  p.y *= 1.2;
  float a = atan(p.y, p.x);
  float r = length(p);
  float fold = r + 0.075 * sin(a * 3.0 + r * 4.0 + time * 0.17);
  float band = exp(-pow((fold - 0.62) / 0.26, 4.0));
  float threads = pow(0.5 + 0.5 * cos(fold * 185.0 + a * 2.0), 12.0);
  float sheen = 0.38 + 0.62 * pow(0.5 + 0.5 * sin(a + 0.8), 2.0);
  vec3 violet = mix(vec3(0.73,0.58,1.0), vec3(0.37,0.22,0.63), light);
  vec3 blue = mix(vec3(0.36,0.79,0.90), vec3(0.12,0.45,0.58), light);
  vec3 gold = mix(vec3(0.91,0.72,0.40), vec3(0.65,0.40,0.12), light);
  vec3 color = mix(violet, blue, smoothstep(-0.2,0.9,sin(a)));
  color = mix(color, gold, smoothstep(0.6,1.0,cos(a - 0.8)) * 0.8);
  float alpha = band * (threads * sheen * 0.9 + 0.035);
  gl_FragColor = vec4(color * alpha, alpha);
}
`;

export function welcomeArt(canvas, surface) {
  let gl, program, buffer, pending = 0, start = 0, last = 0, stopped = false;
  const root = canvas.ownerDocument.documentElement;
  const document = canvas.ownerDocument;
  const motion = matchMedia("(prefers-reduced-motion: reduce)");
  const appearance = matchMedia("(prefers-color-scheme: light)");
  const reduced = () => root.dataset.reducedMotion !== "false" || motion.matches;
  const visible = () => !surface.hidden && !document.hidden && canvas.clientWidth > 0;
  const cancel = () => { cancelAnimationFrame(pending); pending = 0; };
  function create() {
    gl = canvas.getContext("webgl", {alpha:true, antialias:false, depth:false, stencil:false, powerPreference:"low-power"});
    if (!gl) return false;
    const compile = (type, source) => {
      const shader = gl.createShader(type);
      if (!shader) return null;
      gl.shaderSource(shader, source); gl.compileShader(shader);
      return shader;
    };
    const vs = compile(gl.VERTEX_SHADER, vertex), fs = compile(gl.FRAGMENT_SHADER, fragment);
    program = gl.createProgram();
    if (!vs || !fs || !program) {
      if (vs) gl.deleteShader(vs); if (fs) gl.deleteShader(fs);
      if (program) gl.deleteProgram(program);
      return false;
    }
    gl.attachShader(program, vs); gl.attachShader(program, fs);
    gl.bindAttribLocation(program, 0, "position"); gl.linkProgram(program);
    gl.deleteShader(vs); gl.deleteShader(fs);
    if (!gl.getProgramParameter(program, gl.LINK_STATUS)) { gl.deleteProgram(program); program = null; return false; }
    buffer = gl.createBuffer();
    if (!buffer) { gl.deleteProgram(program); program = null; return false; }
    gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1,-1, 3,-1, -1,3]), gl.STATIC_DRAW);
    gl.useProgram(program); gl.enableVertexAttribArray(0); gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);
    program.resolution = gl.getUniformLocation(program, "resolution");
    program.time = gl.getUniformLocation(program, "time");
    program.light = gl.getUniformLocation(program, "light");
    canvas.dataset.renderer = "webgl";
    return true;
  }
  function draw(time) {
    if (!program || !visible() || stopped) return;
    const scale = Math.min(devicePixelRatio || 1, 1.5);
    const width = Math.min(900, Math.round(canvas.clientWidth * scale));
    const height = Math.min(700, Math.round(canvas.clientHeight * scale));
    if (canvas.width !== width || canvas.height !== height) { canvas.width = width; canvas.height = height; }
    gl.viewport(0, 0, width, height);
    gl.uniform2f(program.resolution, width, height);
    gl.uniform1f(program.time, time);
    gl.uniform1f(program.light, root.dataset.theme === "light" || (!root.dataset.theme && appearance.matches) ? 1 : 0);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
    gl.flush();
    canvas.dataset.frames = String(Number(canvas.dataset.frames || 0) + 1);
  }
  function tick(now) {
    pending = 0;
    if (stopped || !visible()) return;
    if (reduced()) { draw(4); return; }
    const elapsed = Math.min((now - start) / 1000, 4);
    if (now - last >= 32 || elapsed === 4) { draw(elapsed); last = now; }
    if (elapsed < 4) pending = requestAnimationFrame(tick);
  }
  function refresh() {
    cancel();
    if (stopped || !visible()) return;
    if (!program && !create()) { canvas.dataset.renderer = "static"; return; }
    start = performance.now(); last = 0;
    if (reduced()) draw(4); else pending = requestAnimationFrame(tick);
  }
  canvas.addEventListener("webglcontextlost", event => {
    event.preventDefault(); cancel(); program = null; buffer = null; stopped = true;
    canvas.dataset.renderer = "static";
  });
  canvas.addEventListener("webglcontextrestored", () => { stopped = false; refresh(); });
  const resize = new ResizeObserver(refresh); resize.observe(canvas);
  const state = new MutationObserver(refresh);
  state.observe(root, {attributes:true, attributeFilter:["data-theme","data-reduced-motion"]});
  state.observe(surface, {attributes:true, attributeFilter:["hidden"]});
  document.addEventListener("visibilitychange", refresh);
  motion.addEventListener("change", refresh); appearance.addEventListener("change", refresh);
  refresh();
  return () => {
    cancel(); stopped = true; resize.disconnect(); state.disconnect();
    document.removeEventListener("visibilitychange", refresh);
    motion.removeEventListener("change", refresh); appearance.removeEventListener("change", refresh);
    if (gl && buffer) gl.deleteBuffer(buffer);
    if (gl && program) gl.deleteProgram(program);
  };
}
