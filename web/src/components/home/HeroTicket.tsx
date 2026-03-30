'use client';

import { useEffect, useRef, useState } from 'react';
import { useTheme } from '@/components/ThemeProvider';

const ZERO_BITMAP = [
  [0, 1, 1, 0],
  [1, 0, 0, 1],
  [1, 0, 0, 1],
  [1, 0, 0, 1],
  [0, 1, 1, 0],
] as const;

const ONE_BITMAP = [
  [0, 1, 0],
  [1, 1, 0],
  [0, 1, 0],
  [0, 1, 0],
  [1, 1, 1],
] as const;

const O_BITMAP = [
  [0, 1, 1, 0],
  [1, 0, 0, 1],
  [1, 0, 0, 1],
  [0, 1, 1, 0],
] as const;

const SIGNAL_RESOLVED_PAYLOAD =
  'FWD://5369676E616C20E28692205265736F6C766564';

function getGlyphBitmap(char: '0' | '1' | 'o') {
  if (char === '0') return ZERO_BITMAP;
  if (char === '1') return ONE_BITMAP;
  return O_BITMAP;
}

function TicketCanvas({
  backgroundImageSrc,
  isDark,
  onMeshReady,
}: {
  backgroundImageSrc: string;
  isDark: boolean;
  onMeshReady?: () => void;
}) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const animFrameRef = useRef<number>(0);
  const isDarkRef = useRef(isDark);
  const onMeshReadyRef = useRef(onMeshReady);
  isDarkRef.current = isDark;
  onMeshReadyRef.current = onMeshReady;

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const canvasEl = canvas;
    const container = canvas.parentElement;
    if (!container) return;
    const host = container;

    const context = canvasEl.getContext('2d');
    if (!context) return;
    const ctx: CanvasRenderingContext2D = context;
    let meshReadySent = false;

    let glyphs: Array<{
      char: '0' | '1' | 'o';
      contourX: number;
      contourY: number;
      darkness: number;
      detail: number;
      flowScale: number;
      opacityBase: number;
      phase: number;
      pixelSize: number;
      sprite: HTMLCanvasElement;
      threshold: number;
      x: number;
      y: number;
    }> = [];
    let imageSample: { data: Uint8ClampedArray; height: number; width: number } | null = null;
    const spriteCache = new Map<string, HTMLCanvasElement>();
    const sampleCanvas = document.createElement('canvas');
    const sampleContext = sampleCanvas.getContext('2d');

    function getSprite(char: '0' | '1' | 'o', pixelSize: number) {
      const key = `${char}:${pixelSize}`;
      const cached = spriteCache.get(key);
      if (cached) {
        return cached;
      }

      const bitmap = getGlyphBitmap(char);
      const sprite = document.createElement('canvas');
      sprite.width = bitmap[0].length * pixelSize;
      sprite.height = bitmap.length * pixelSize;

      const spriteContext = sprite.getContext('2d');
      if (!spriteContext) {
        return sprite;
      }

      spriteContext.fillStyle = '#ffffff';
      for (let row = 0; row < bitmap.length; row += 1) {
        for (let column = 0; column < bitmap[row].length; column += 1) {
          if (!bitmap[row][column]) {
            continue;
          }
          spriteContext.fillRect(
            column * pixelSize,
            row * pixelSize,
            pixelSize,
            pixelSize
          );
        }
      }

      spriteCache.set(key, sprite);
      return sprite;
    }

    function sampleLuminance(x: number, y: number) {
      if (!imageSample) {
        return 0.5;
      }

      const clampedX = Math.max(0, Math.min(imageSample.width - 1, Math.round(x)));
      const clampedY = Math.max(0, Math.min(imageSample.height - 1, Math.round(y)));
      const index = (clampedY * imageSample.width + clampedX) * 4;
      const red = imageSample.data[index] / 255;
      const green = imageSample.data[index + 1] / 255;
      const blue = imageSample.data[index + 2] / 255;
      return red * 0.2126 + green * 0.7152 + blue * 0.0722;
    }

    function sampleImageInfo(containerX: number, containerY: number, rect: DOMRect) {
      if (!imageSample) {
        return {
          contourX: 1,
          contourY: 0,
          detail: 0.3,
          luminance: 0.5,
        };
      }

      const scale = Math.max(
        rect.width / imageSample.width,
        rect.height / imageSample.height
      );
      const drawWidth = imageSample.width * scale;
      const drawHeight = imageSample.height * scale;
      const offsetX = (rect.width - drawWidth) / 2;
      const offsetY = (rect.height - drawHeight) / 2;
      const imageX = (containerX - offsetX) / scale;
      const imageY = (containerY - offsetY) / scale;

      const luminance = sampleLuminance(imageX, imageY);
      const left = sampleLuminance(imageX - 1.2, imageY);
      const right = sampleLuminance(imageX + 1.2, imageY);
      const top = sampleLuminance(imageX, imageY - 1.2);
      const bottom = sampleLuminance(imageX, imageY + 1.2);
      const signedGradientX = right - left;
      const signedGradientY = bottom - top;
      const gradientX = Math.abs(signedGradientX);
      const gradientY = Math.abs(signedGradientY);
      const diagonal = Math.abs(
        sampleLuminance(imageX + 1, imageY + 1)
        - sampleLuminance(imageX - 1, imageY - 1)
      );
      const edge = Math.min(1, (gradientX + gradientY + diagonal * 0.7) * 1.8);
      const contrast = Math.abs(luminance - 0.5) * 2;
      const contourLength = Math.hypot(signedGradientX, signedGradientY);
      const contourX = contourLength > 0.0001 ? -signedGradientY / contourLength : 1;
      const contourY = contourLength > 0.0001 ? signedGradientX / contourLength : 0;

      return {
        contourX,
        contourY,
        detail: Math.max(0.08, Math.min(1, edge * 0.78 + contrast * 0.34)),
        luminance,
      };
    }

    function resizeCanvas() {
      const rect = host.getBoundingClientRect();
      if (rect.width === 0 || rect.height === 0) {
        return;
      }
      const dpr = window.devicePixelRatio || 1;
      canvasEl.width = rect.width * dpr;
      canvasEl.height = rect.height * dpr;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

      const cellSize = Math.max(5, Math.round(Math.min(rect.width, rect.height) / 56));
      const columns = Math.ceil(rect.width / cellSize) + 3;
      const rows = Math.ceil(rect.height / cellSize) + 3;

      glyphs = [];
      for (let row = 0; row < rows; row += 1) {
        for (let column = 0; column < columns; column += 1) {
          const seed = (column * 17 + row * 31) % 11;
          const x = column * cellSize + cellSize * 0.16;
          const y = row * cellSize + cellSize * 0.14;
          const info = sampleImageInfo(x, y, rect);
          const detail = info.detail;
          const luminance = info.luminance;
          const darkness = 1 - luminance;
          const shadowPenalty = Math.max(0, darkness - 0.48) / 0.52;
          const shadowFade = Math.min(1, Math.pow(shadowPenalty, 1.22) * 1.32);
          const char =
            detail > 0.6
              ? seed % 5 === 0
                ? 'o'
                : luminance > 0.46
                  ? '1'
                  : seed % 4 === 0
                    ? '0'
                    : '1'
              : luminance > 0.58
                ? '1'
              : luminance < 0.34
                ? seed % 3 === 0
                  ? '0'
                  : '1'
                : seed % 6 === 0
                  ? 'o'
                  : '1';
          const pixelSize = 1;
          glyphs.push({
            char,
            contourX: info.contourX,
            contourY: info.contourY,
            darkness,
            detail,
            flowScale: 0.42 + detail * 1.55,
            opacityBase:
              0.058
              + detail * (0.24 + (1 - shadowFade) * 0.14)
              + (seed % 4) * 0.012,
            phase: column * 0.37 + row * 0.21 + seed * 0.16,
            pixelSize,
            sprite: getSprite(char, pixelSize),
            threshold:
              0.022
              + (1 - detail) * 0.04
              + luminance * 0.014
              + shadowFade * 0.16,
            x,
            y,
          });
        }
      }
    }

    const resizeObserver = new ResizeObserver(() => {
      resizeCanvas();
    });
    resizeObserver.observe(host);
    window.addEventListener('resize', resizeCanvas);
    resizeCanvas();

    const posterImage = new Image();
    posterImage.src = backgroundImageSrc;
    posterImage.decoding = 'async';
    posterImage.onload = () => {
      if (!sampleContext) {
        return;
      }

      sampleCanvas.width = posterImage.naturalWidth;
      sampleCanvas.height = posterImage.naturalHeight;
      sampleContext.drawImage(posterImage, 0, 0);
      const pixels = sampleContext.getImageData(
        0,
        0,
        posterImage.naturalWidth,
        posterImage.naturalHeight
      );
      imageSample = {
        data: pixels.data,
        height: pixels.height,
        width: pixels.width,
      };
      resizeCanvas();
    };

    function render(time: number) {
      const rect = host.getBoundingClientRect();
      const now = time * 0.001;
      const breath = Math.sin(now * 0.2) * 0.5 + 0.5;
      ctx.clearRect(0, 0, rect.width, rect.height);
      ctx.fillStyle = isDarkRef.current
        ? 'rgba(3, 3, 3, 0.12)'
        : 'rgba(255, 255, 255, 0.07)';
      ctx.fillRect(0, 0, rect.width, rect.height);

      for (const glyph of glyphs) {
        const shimmer =
          Math.sin(glyph.x * 0.028 + now * 0.62 + glyph.phase) * 0.36
          + Math.cos(glyph.y * 0.024 - now * 0.36 + glyph.phase) * 0.3
          + Math.sin((glyph.x + glyph.y) * 0.016 - now * 0.24 + glyph.phase) * 0.26;
        const cluster =
          Math.sin(now * 0.36 + glyph.y * 0.012 + glyph.phase) * 0.075
          + Math.cos(now * 0.28 - glyph.x * 0.01 + glyph.phase * 0.7) * 0.065;
        const pulse =
          Math.sin(now * 0.5 + glyph.phase * 1.3 + glyph.y * 0.01) * 0.04
          + breath * 0.024;
        const contourGlow =
          Math.sin(
            now * 0.34
            + glyph.phase
            + glyph.x * glyph.contourX * 0.02
            + glyph.y * glyph.contourY * 0.02
          ) * 0.06
          + glyph.detail * 0.05;
        const intensity = Math.max(
          0,
          Math.min(1, glyph.opacityBase + shimmer * 0.1 + cluster + pulse + contourGlow)
        );
        if (intensity < glyph.threshold) {
          continue;
        }

        const shadowSuppression = Math.max(0, glyph.darkness - 0.48) / 0.52;
        const shadowFade = Math.min(1, Math.pow(shadowSuppression, 1.24) * 1.34);
        const shadowGain = 1 - shadowFade * 0.94;
        const opacity = isDarkRef.current
          ? 0.016 + shadowGain * 0.036
            + intensity
              * shadowGain
              * (0.24 + glyph.detail * 0.16 + (1 - shadowFade) * 0.14)
          : 0.012 + shadowGain * 0.022
            + intensity
              * shadowGain
              * (0.16 + glyph.detail * 0.11 + (1 - shadowFade) * 0.09);
        const streamX =
          Math.sin(now * 0.26 + glyph.y * 0.018 + glyph.phase) * 1.12
          + Math.cos(now * 0.2 - glyph.x * 0.013 + glyph.phase) * 0.9;
        const streamY =
          Math.cos(now * 0.24 + glyph.x * 0.015 + glyph.phase) * 0.94
          + Math.sin(now * 0.18 - glyph.y * 0.011 + glyph.phase * 1.2) * 0.78;
        const swirlX =
          Math.sin(now * 0.14 + glyph.x * 0.009 - glyph.y * 0.007 + glyph.phase) * 1.24
          + Math.cos(now * 0.1 + glyph.y * 0.012 + glyph.phase * 0.8) * 0.82;
        const swirlY =
          Math.cos(now * 0.13 + glyph.y * 0.01 - glyph.x * 0.008 + glyph.phase) * 1.14
          - Math.sin(now * 0.1 + glyph.x * 0.011 + glyph.phase * 0.9) * 0.76;
        const curl =
          Math.sin(now * 0.31 + (glyph.x - glyph.y) * 0.01 + glyph.phase) * 0.52
          + Math.cos(now * 0.23 + (glyph.x + glyph.y) * 0.008 + glyph.phase) * 0.4;
        const contourWave =
          Math.sin(
            now * 0.4
            + glyph.phase * 1.1
            + glyph.x * glyph.contourX * 0.018
            + glyph.y * glyph.contourY * 0.018
          ) * 1.62
          + Math.cos(
            now * 0.25
            - glyph.phase * 0.8
            + glyph.x * glyph.contourY * 0.012
            - glyph.y * glyph.contourX * 0.012
          ) * 1.04;
        const contourPulse =
          Math.cos(
            now * 0.21
            + glyph.phase
            + glyph.x * glyph.contourY * 0.01
            - glyph.y * glyph.contourX * 0.01
          ) * 0.58;
        const contourWeight = 0.3 + glyph.detail * 1.15;
        const contourDriftX =
          glyph.contourX * contourWave * glyph.flowScale * contourWeight
          - glyph.contourY * contourPulse * glyph.flowScale * 0.32;
        const contourDriftY =
          glyph.contourY * contourWave * glyph.flowScale * contourWeight
          + glyph.contourX * contourPulse * glyph.flowScale * 0.32;
        const ambientWeight = 0.9 - glyph.detail * 0.45;
        const driftX =
          (
            (streamX * 0.6 + swirlX * 0.4 + curl * 0.7) * ambientWeight
            + contourDriftX
          ) * (0.85 + breath * 0.35);
        const driftY =
          (
            (streamY * 0.58 + swirlY * 0.42 - curl * 0.55) * ambientWeight
            + contourDriftY
          ) * (0.85 + breath * 0.35);

        ctx.globalAlpha = opacity;
        ctx.drawImage(glyph.sprite, glyph.x + driftX, glyph.y + driftY);
        ctx.globalAlpha = opacity * (0.4 + glyph.detail * 0.18);
        ctx.drawImage(glyph.sprite, glyph.x + driftX * 1.52, glyph.y + driftY * 1.52);
        ctx.globalAlpha = opacity * 0.3;
        ctx.drawImage(glyph.sprite, glyph.x - driftX * 0.82, glyph.y - driftY * 0.82);
      }
      ctx.globalAlpha = 1;

      if (!meshReadySent && glyphs.length > 0) {
        meshReadySent = true;
        onMeshReadyRef.current?.();
      }

      animFrameRef.current = requestAnimationFrame(render);
    }

    animFrameRef.current = requestAnimationFrame(render);

    return () => {
      resizeObserver.disconnect();
      window.removeEventListener('resize', resizeCanvas);
      if (animFrameRef.current) {
        cancelAnimationFrame(animFrameRef.current);
      }
    };
  }, [backgroundImageSrc]);

  return (
    <canvas
      ref={canvasRef}
      style={{
        position: 'absolute',
        inset: 0,
        width: '100%',
        height: '100%',
        display: 'block',
        zIndex: 1,
        pointerEvents: 'none',
      }}
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

export interface HeroTicketRow {
  label: string;
  value: string;
}

interface HeroTicketProps {
  accessValue?: string;
  statusValue?: string;
  networkValue?: string;
  modeValue?: string;
  detailRows?: HeroTicketRow[];
  backgroundImageSrc: string;
}

const DEFAULT_DETAIL_ROWS: HeroTicketRow[] = [
  { label: 'AGENTS', value: 'NONE LIVE' },
  { label: 'MARKETS', value: '0 TRACKED' },
  { label: 'FEEDS', value: '0/0 LIVE' },
];

export function HeroTicket({
  accessValue = 'PUBLIC WEB',
  statusValue = 'LIVE',
  networkValue = 'BASE L2',
  modeValue = 'MARKET MONITOR',
  detailRows = DEFAULT_DETAIL_ROWS,
  backgroundImageSrc,
}: HeroTicketProps) {
  const { resolvedTheme } = useTheme();
  const isDark = resolvedTheme === 'dark';
  const [isMeshReady, setIsMeshReady] = useState(false);
  const [isImageVisible, setIsImageVisible] = useState(false);

  const dataGrid: React.CSSProperties = {
    display: 'grid',
    gridTemplateColumns: '80px 20px 1fr',
    rowGap: '6px',
  };

  useEffect(() => {
    setIsMeshReady(false);
    setIsImageVisible(false);
  }, [backgroundImageSrc]);

  useEffect(() => {
    if (!isMeshReady) {
      return;
    }

    const timeout = window.setTimeout(() => {
      setIsImageVisible(true);
    }, 140);

    return () => {
      window.clearTimeout(timeout);
    };
  }, [isMeshReady]);

  return (
    <div
      className="relative flex h-full w-full flex-col overflow-hidden bg-bg-secondary text-text-primary sm:flex-row"
      style={{ fontFamily: 'var(--font-mono)' }}
    >
      <div
        className="relative min-h-[200px] w-full flex-[1_1_50%] overflow-hidden border-b border-border sm:min-h-0 sm:border-b-0 sm:border-r"
        style={{ backgroundColor: isDark ? '#030303' : '#f4f1ea' }}
      >
        <div
          aria-hidden
          style={{
            position: 'absolute',
            inset: 0,
            backgroundImage: `url('${backgroundImageSrc}')`,
            backgroundSize: 'cover',
            backgroundPosition: 'center',
            opacity: isImageVisible ? 1 : 0,
            transition: 'opacity 420ms ease',
            zIndex: 0,
          }}
        />
        <TicketCanvas
          backgroundImageSrc={backgroundImageSrc}
          isDark={isDark}
          onMeshReady={() => setIsMeshReady(true)}
        />
      </div>

      <div
        className="z-[2] flex flex-[1_1_50%] flex-col bg-bg-secondary"
        style={{
          padding: '2rem 1.5rem 1.5rem 1.5rem',
          fontSize: '0.75rem',
          lineHeight: 1.2,
        }}
      >
        <div style={dataGrid}>
          <DataRow label="TYPE" value="PREDICTION MARKET" />
          <DataRow label="ACCESS" value={accessValue} />
          <div style={{ gridColumn: '1/-1', height: '8px' }} />
          <DataRow label="STATUS" value={statusValue} />
          <DataRow label="NETWORK" value={networkValue} />
          <DataRow label="MODE" value={modeValue} />
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
          {SIGNAL_RESOLVED_PAYLOAD}
        </div>

        <div style={dataGrid}>
          {detailRows.slice(0, 3).map((row) => (
            <DataRow key={row.label} label={row.label} value={row.value} />
          ))}
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
          <span>
            SIGNAL → <span style={{ color: '#22c55e' }}>RESOLVED</span>
          </span>
        </div>
      </div>
    </div>
  );
}
