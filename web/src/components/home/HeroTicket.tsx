'use client';

import { useEffect, useRef, useState } from 'react';
import { useTheme } from '@/components/ThemeProvider';

function TicketCanvas({ isDark }: { isDark: boolean }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const animFrameRef = useRef<number>(0);
  const isDarkRef = useRef(isDark);
  isDarkRef.current = isDark;

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const gl = canvas.getContext('webgl', { antialias: false });
    if (!gl) return;

    function resizeCanvas() {
      const rect = canvas!.parentElement!.getBoundingClientRect();
      const dpr = window.devicePixelRatio || 1;
      canvas!.width = rect.width * dpr;
      canvas!.height = rect.height * dpr;
      gl!.viewport(0, 0, canvas!.width, canvas!.height);
    }

    window.addEventListener('resize', resizeCanvas);
    resizeCanvas();

    const vsSource = `
      attribute vec4 aVertexPosition;
      void main() {
        gl_Position = aVertexPosition;
      }
    `;

    const fsSource = `
      precision highp float;
      uniform vec2 u_resolution;
      uniform float u_time;
      uniform vec3 u_dotColor;

      vec3 permute(vec3 x) { return mod(((x*34.0)+1.0)*x, 289.0); }
      float snoise(vec2 v){
        const vec4 C = vec4(0.211324865405187, 0.366025403784439, -0.577350269189626, 0.024390243902439);
        vec2 i  = floor(v + dot(v, C.yy));
        vec2 x0 = v - i + dot(i, C.xx);
        vec2 i1;
        i1 = (x0.x > x0.y) ? vec2(1.0, 0.0) : vec2(0.0, 1.0);
        vec4 x12 = x0.xyxy + C.xxzz;
        x12.xy -= i1;
        i = mod(i, 289.0);
        vec3 p = permute(permute(i.y + vec3(0.0, i1.y, 1.0)) + i.x + vec3(0.0, i1.x, 1.0));
        vec3 m = max(0.5 - vec3(dot(x0,x0), dot(x12.xy,x12.xy), dot(x12.zw,x12.zw)), 0.0);
        m = m*m; m = m*m;
        vec3 x = 2.0 * fract(p * C.www) - 1.0;
        vec3 h = abs(x) - 0.5;
        vec3 ox = floor(x + 0.5);
        vec3 a0 = x - ox;
        m *= 1.79284291400159 - 0.85373472095314 * (a0*a0 + h*h);
        vec3 g;
        g.x  = a0.x  * x0.x  + h.x  * x0.y;
        g.yz = a0.yz * x12.xz + h.yz * x12.yw;
        return 130.0 * dot(m, g);
      }

      void main() {
        vec2 st = gl_FragCoord.xy/u_resolution.xy;
        st.x *= u_resolution.x/u_resolution.y;

        float scale = 60.0;
        vec2 gridUv = st * scale;
        vec2 id = floor(gridUv);
        vec2 fractUv = fract(gridUv) - 0.5;

        float t = u_time * 0.15;
        float n1 = snoise(id * 0.08 + vec2(t, t*0.3));
        float finalNoise = n1 * 0.5 + 0.5;

        float targetRadius = smoothstep(0.3, 0.7, finalNoise) * 0.42;
        float dist = length(fractUv);
        float edge = 0.06;
        float dotAlpha = 1.0 - smoothstep(targetRadius - edge, targetRadius + edge, dist);

        gl_FragColor = vec4(u_dotColor, dotAlpha * 0.9);
      }
    `;

    function createShader(glCtx: WebGLRenderingContext, type: number, source: string) {
      const shader = glCtx.createShader(type)!;
      glCtx.shaderSource(shader, source);
      glCtx.compileShader(shader);
      return shader;
    }

    const vertexShader = createShader(gl, gl.VERTEX_SHADER, vsSource);
    const fragmentShader = createShader(gl, gl.FRAGMENT_SHADER, fsSource);
    const program = gl.createProgram()!;
    gl.attachShader(program, vertexShader);
    gl.attachShader(program, fragmentShader);
    gl.linkProgram(program);

    const positionBuffer = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, positionBuffer);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, 1, 1, 1, -1, -1, 1, -1]), gl.STATIC_DRAW);

    const positionLocation = gl.getAttribLocation(program, 'aVertexPosition');
    const resolutionLocation = gl.getUniformLocation(program, 'u_resolution');
    const timeLocation = gl.getUniformLocation(program, 'u_time');
    const dotColorLocation = gl.getUniformLocation(program, 'u_dotColor');

    function render(time: number) {
      time *= 0.001;
      gl!.viewport(0, 0, canvas!.width, canvas!.height);
      gl!.clearColor(0, 0, 0, 0);
      gl!.clear(gl!.COLOR_BUFFER_BIT);
      gl!.useProgram(program);
      gl!.enableVertexAttribArray(positionLocation);
      gl!.bindBuffer(gl!.ARRAY_BUFFER, positionBuffer);
      gl!.vertexAttribPointer(positionLocation, 2, gl!.FLOAT, false, 0, 0);
      gl!.uniform2f(resolutionLocation, canvas!.width, canvas!.height);
      gl!.uniform1f(timeLocation, time);
      if (isDarkRef.current) {
        gl!.uniform3f(dotColorLocation, 0.91, 0.89, 0.86);
      } else {
        gl!.uniform3f(dotColorLocation, 0.15, 0.15, 0.15);
      }
      gl!.enable(gl!.BLEND);
      gl!.blendFunc(gl!.SRC_ALPHA, gl!.ONE_MINUS_SRC_ALPHA);
      gl!.drawArrays(gl!.TRIANGLE_STRIP, 0, 4);
      animFrameRef.current = requestAnimationFrame(render);
    }

    animFrameRef.current = requestAnimationFrame(render);

    return () => {
      window.removeEventListener('resize', resizeCanvas);
      if (animFrameRef.current) {
        cancelAnimationFrame(animFrameRef.current);
      }
    };
  }, []);

  return (
    <canvas
      ref={canvasRef}
      style={{ width: '100%', height: '100%', display: 'block' }}
    />
  );
}

interface DataRowProps {
  label: string;
  value: string;
}

function DataRow({ label, value }: DataRowProps) {
  return (
    <>
      <div style={{ gridColumn: 1, opacity: 0.6 }}>{label}</div>
      <div style={{ gridColumn: 2, opacity: 0.4 }}>&gt;</div>
      <div style={{ gridColumn: 3, opacity: 0.9 }}>{value}</div>
    </>
  );
}

export function HeroTicket() {
  const [isDesktop, setIsDesktop] = useState(false);
  const { resolvedTheme } = useTheme();
  const isDark = resolvedTheme === 'dark';

  useEffect(() => {
    const check = () => setIsDesktop(window.innerWidth >= 600);
    check();
    window.addEventListener('resize', check);
    return () => window.removeEventListener('resize', check);
  }, []);

  const borderColor = isDark ? 'rgba(232, 228, 220, 0.1)' : 'rgba(0, 0, 0, 0.1)';

  const dataGrid: React.CSSProperties = {
    display: 'grid',
    gridTemplateColumns: '80px 20px 1fr',
    rowGap: '6px',
  };

  return (
    <div
      className="bg-bg-secondary text-text-primary"
      style={{
        position: 'relative',
        display: 'flex',
        flexDirection: isDesktop ? 'row' : 'column',
        overflow: 'hidden',
        width: '100%',
        height: '100%',
        fontFamily: "var(--font-mono)",
      }}
    >
      {/* Canvas — left half */}
      <div
        style={{
          flex: '1 1 50%',
          width: '100%',
          position: 'relative',
          overflow: 'hidden',
          borderRight: isDesktop ? `1px solid ${borderColor}` : 'none',
          borderBottom: isDesktop ? 'none' : `1px solid ${borderColor}`,
          minHeight: isDesktop ? 'auto' : '200px',
        }}
      >
        <div
          className="text-text-primary"
          style={{
            position: 'absolute',
            top: '1.5rem',
            right: '1.5rem',
            textAlign: 'right',
            fontSize: '0.7rem',
            lineHeight: 1.4,
            letterSpacing: '2px',
            opacity: 0.8,
            pointerEvents: 'none',
            zIndex: 10,
          }}
        >
          <span style={{ display: 'block' }}>R E L A Y</span>
          <span style={{ display: 'block', paddingLeft: '1.5rem' }}>4 4</span>
          <span
            style={{
              display: 'block',
              paddingLeft: '0.5rem',
              marginTop: '8px',
              fontSize: '0.6rem',
              opacity: 0.5,
            }}
          >
            S I G N A L &nbsp; N E T W O R K
          </span>
        </div>
        <TicketCanvas isDark={isDark} />
      </div>

      {/* Data — right half */}
      <div
        className="bg-bg-secondary"
        style={{
          flex: '1 1 50%',
          padding: '2rem 1.5rem 1.5rem 1.5rem',
          display: 'flex',
          flexDirection: 'column',
          fontSize: '0.75rem',
          lineHeight: 1.2,
          zIndex: 2,
        }}
      >
        <div style={dataGrid}>
          <DataRow label="TYPE" value="PREDICTION MARKET" />
          <DataRow label="ACCESS" value="ALL CHAINS" />
          <div style={{ gridColumn: '1/-1', height: '8px' }} />
          <DataRow label="STATUS" value="LIVE" />
          <DataRow label="NETWORK" value="BASE L2" />
          <DataRow label="MODE" value="AUTONOMOUS AGENTS" />
        </div>

        <div
          className="opacity-20"
          style={{
            whiteSpace: 'nowrap',
            overflow: 'hidden',
            lineHeight: 1,
            margin: '1.2rem 0',
            letterSpacing: '1px',
            fontSize: '0.7rem',
          }}
        >
          LLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLL
        </div>

        <div style={dataGrid}>
          <DataRow label="AGENT 01" value="OSPREY-7" />
          <DataRow label="AGENT 02" value="MANTIS-V" />
          <DataRow label="AGENT 03" value="KESTREL-3" />
        </div>

        <div
          className="border-t border-border"
          style={{
            marginTop: 'auto',
            paddingTop: '1rem',
            fontSize: '0.75rem',
            textTransform: 'uppercase',
          }}
        >
          <span>RELAY44</span>
          <span className="inline-block w-1 h-1 bg-text-primary opacity-50 mx-2 align-[2px]" />
          <span>SIGNAL → RESOLVED</span>
        </div>
      </div>
    </div>
  );
}
