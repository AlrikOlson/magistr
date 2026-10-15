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
mat2 turn(float a) { return mat2(cos(a), -sin(a), sin(a), cos(a)); }
float sculpture(vec3 p) {
  p.xy = turn(-0.38) * p.xy;
  p.yz = turn(0.72) * p.yz;
  p.xz = turn(0.25 + 0.12 * (1.0 - pow(1.0 - time / 4.0, 3.0))) * p.xz;
  float angle = atan(p.z, p.x);
  vec2 band = vec2(length(p.xz) - 0.69, p.y);
  band = turn(angle * 1.5) * band;
  vec2 edge = abs(band) - vec2(0.24, 0.075);
  return (length(max(edge, 0.0)) + min(max(edge.x, edge.y), 0.0) - 0.045) * 0.7;
}
vec3 normalAt(vec3 p) {
  vec2 e = vec2(0.004, -0.004);
  return normalize(e.xyy * sculpture(p + e.xyy) + e.yyx * sculpture(p + e.yyx)
    + e.yxy * sculpture(p + e.yxy) + e.xxx * sculpture(p + e.xxx));
}
vec3 environment(vec3 r) {
  vec3 ink = vec3(0.12, 0.19, 0.33);
  vec3 pearl = vec3(0.90, 0.92, 1.0);
  vec3 c = mix(ink, pearl, smoothstep(-0.65, 0.65, r.y));
  c = mix(c, vec3(0.68, 0.58, 1.0), smoothstep(0.15, 0.92, r.x) * 0.7);
  c = mix(c, vec3(0.97, 0.72, 0.40), smoothstep(0.3, 0.95, -r.x) * 0.65);
  c += vec3(0.55, 0.68, 0.85) * pow(max(0.0, dot(r, normalize(vec3(0.4, 0.8, 0.5)))), 24.0);
  return c;
}
void main() {
  vec2 uv = (gl_FragCoord.xy * 2.0 - resolution) / min(resolution.x, resolution.y);
  vec3 origin = vec3(0.0, 0.0, 3.5);
  vec3 ray = normalize(vec3(uv * 1.24, -2.8));
  float travel = 0.0;
  float distance = 1.0;
  for (int i = 0; i < 72; i++) {
    distance = sculpture(origin + ray * travel);
    if (distance < 0.0025 || travel > 5.0) break;
    travel += distance;
  }
  vec3 color;
  float alpha;
  if (travel < 5.0 && distance < 0.0025) {
    vec3 p = origin + ray * travel;
    vec3 n = normalAt(p);
    vec3 reflected = reflect(ray, n);
    float diffuse = max(0.0, dot(n, normalize(vec3(-0.4, 0.7, 1.0))));
    float fresnel = pow(1.0 - max(0.0, dot(n, -ray)), 3.0);
    float occlusion = clamp(sculpture(p + n * 0.10) / 0.07, 0.25, 1.0);
    color = environment(reflected) * (0.54 + 0.46 * diffuse) * occlusion;
    color += fresnel * vec3(0.22, 0.27, 0.40);
    color *= 0.97 + 0.03 * sin(atan(p.z, p.x) * 220.0);
    color = mix(color, color * 0.84 + vec3(0.04), light);
    color = pow(clamp(color, 0.0, 1.0), vec3(0.8));
    alpha = 1.0;
  } else {
    float shadow = exp(-dot(uv * vec2(1.2, 5.0) + vec2(0.0, 4.2),
      uv * vec2(1.2, 5.0) + vec2(0.0, 4.2)));
    color = vec3(0.02, 0.03, 0.08);
    vec2 edge = min(gl_FragCoord.xy / resolution, 1.0 - gl_FragCoord.xy / resolution);
    alpha = shadow * mix(0.35, 0.14, light) * smoothstep(0.0, 0.18, min(edge.x, edge.y));
  }
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
    const width = Math.min(640, Math.round(canvas.clientWidth * scale));
    const height = Math.min(640, Math.round(canvas.clientHeight * scale));
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