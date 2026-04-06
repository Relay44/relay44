'use client';

import { useState, useCallback, useRef, useEffect } from 'react';
import { Card } from '@/components/ui/Card';
import { Badge } from '@/components/ui/Badge';
import type { ContextGraphResult, GraphNode, GraphEdge, GraphNodeType } from './types';
import { NODE_COLORS, EDGE_COLORS } from './types';
import { MisinfoScoreCard } from './MisinfoScoreCard';
import { ClaimCard } from './ClaimCard';
import { SourceCredibilityBar } from './SourceCredibilityBar';

interface Props {
  result: ContextGraphResult;
}

interface PositionedNode extends GraphNode {
  x: number;
  y: number;
}

export function ContextGraphViewer({ result }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [selectedNode, setSelectedNode] = useState<GraphNode | null>(null);
  const [positions, setPositions] = useState<Map<string, { x: number; y: number }>>(new Map());
  const [hoveredNode, setHoveredNode] = useState<string | null>(null);

  const claims = result.nodes.filter((n) => n.type === 'claim');
  const sources = result.nodes.filter((n) => n.type === 'source');

  // Force-directed layout
  useEffect(() => {
    const pos = new Map<string, { x: number; y: number }>();
    const width = 600;
    const height = 400;
    const centerX = width / 2;
    const centerY = height / 2;

    const typeOrder: GraphNodeType[] = ['market', 'claim', 'source'];
    const typeGroups = new Map<GraphNodeType, GraphNode[]>();

    for (const node of result.nodes) {
      const group = typeGroups.get(node.type) || [];
      group.push(node);
      typeGroups.set(node.type, group);
    }

    for (const [type, nodes] of typeGroups) {
      const ringIdx = typeOrder.indexOf(type);
      const radius = (ringIdx + 1) * 100;

      nodes.forEach((node, i) => {
        const angle = (2 * Math.PI * i) / nodes.length;
        pos.set(node.id, {
          x: centerX + radius * Math.cos(angle),
          y: centerY + radius * Math.sin(angle),
        });
      });
    }

    for (let iter = 0; iter < 20; iter++) {
      const nodeIds = Array.from(pos.keys());
      for (let i = 0; i < nodeIds.length; i++) {
        for (let j = i + 1; j < nodeIds.length; j++) {
          const a = pos.get(nodeIds[i])!;
          const b = pos.get(nodeIds[j])!;
          const dx = b.x - a.x;
          const dy = b.y - a.y;
          const dist = Math.sqrt(dx * dx + dy * dy) || 1;
          const force = 500 / (dist * dist);

          a.x -= (dx / dist) * force;
          a.y -= (dy / dist) * force;
          b.x += (dx / dist) * force;
          b.y += (dy / dist) * force;
        }
      }

      for (const edge of result.edges) {
        const a = pos.get(edge.source);
        const b = pos.get(edge.target);
        if (!a || !b) continue;

        const dx = b.x - a.x;
        const dy = b.y - a.y;
        const dist = Math.sqrt(dx * dx + dy * dy) || 1;
        const force = dist * 0.01;

        a.x += dx * force;
        a.y += dy * force;
        b.x -= dx * force;
        b.y -= dy * force;
      }

      for (const p of pos.values()) {
        p.x += (centerX - p.x) * 0.01;
        p.y += (centerY - p.y) * 0.01;
      }
    }

    setPositions(pos);
  }, [result]);

  // Canvas rendering
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || positions.size === 0) return;

    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const dpr = window.devicePixelRatio || 1;
    canvas.width = canvas.offsetWidth * dpr;
    canvas.height = canvas.offsetHeight * dpr;
    ctx.scale(dpr, dpr);

    const w = canvas.offsetWidth;
    const h = canvas.offsetHeight;

    ctx.clearRect(0, 0, w, h);

    for (const edge of result.edges) {
      const from = positions.get(edge.source);
      const to = positions.get(edge.target);
      if (!from || !to) continue;

      ctx.beginPath();
      ctx.moveTo(from.x, from.y);
      ctx.lineTo(to.x, to.y);
      ctx.strokeStyle = EDGE_COLORS[edge.type] || '#333';
      ctx.lineWidth = Math.max(0.5, edge.weight * 2);
      ctx.globalAlpha = 0.4;
      ctx.stroke();
      ctx.globalAlpha = 1;
    }

    for (const node of result.nodes) {
      const pos = positions.get(node.id);
      if (!pos) continue;

      const isHovered = hoveredNode === node.id;
      const isSelected = selectedNode?.id === node.id;
      const radius = node.type === 'market' ? 12 : node.type === 'claim' ? 8 : 5;
      const color = NODE_COLORS[node.type];

      if (isHovered || isSelected) {
        ctx.beginPath();
        ctx.arc(pos.x, pos.y, radius + 4, 0, Math.PI * 2);
        ctx.fillStyle = `${color}40`;
        ctx.fill();
      }

      ctx.beginPath();
      ctx.arc(pos.x, pos.y, radius, 0, Math.PI * 2);
      ctx.fillStyle = color;
      ctx.fill();

      if (node.type === 'market') {
        ctx.fillStyle = '#fff';
        ctx.font = '10px system-ui';
        ctx.textAlign = 'center';
        ctx.fillText(node.label.slice(0, 40), pos.x, pos.y + radius + 14);
      }
    }
  }, [positions, result, hoveredNode, selectedNode]);

  const handleCanvasClick = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>) => {
      const canvas = canvasRef.current;
      if (!canvas) return;

      const rect = canvas.getBoundingClientRect();
      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;

      for (const node of result.nodes) {
        const pos = positions.get(node.id);
        if (!pos) continue;

        const radius = node.type === 'market' ? 12 : node.type === 'claim' ? 8 : 5;
        const dx = pos.x - x;
        const dy = pos.y - y;

        if (dx * dx + dy * dy < (radius + 4) * (radius + 4)) {
          setSelectedNode(node);
          return;
        }
      }

      setSelectedNode(null);
    },
    [result.nodes, positions],
  );

  const handleMouseMove = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>) => {
      const canvas = canvasRef.current;
      if (!canvas) return;

      const rect = canvas.getBoundingClientRect();
      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;

      for (const node of result.nodes) {
        const pos = positions.get(node.id);
        if (!pos) continue;

        const radius = node.type === 'market' ? 12 : 8;
        const dx = pos.x - x;
        const dy = pos.y - y;

        if (dx * dx + dy * dy < (radius + 4) * (radius + 4)) {
          setHoveredNode(node.id);
          canvas.style.cursor = 'pointer';
          return;
        }
      }

      setHoveredNode(null);
      canvas.style.cursor = 'default';
    },
    [result.nodes, positions],
  );

  return (
    <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
      {/* Graph canvas */}
      <Card className="lg:col-span-2 p-0 overflow-hidden">
        <div className="p-3 border-b border-border flex items-center justify-between">
          <h2 className="text-sm font-medium text-text-primary">Context Graph</h2>
          <div className="flex items-center gap-3 text-[10px] text-text-muted">
            <span className="flex items-center gap-1">
              <span className="w-2 h-2 rounded-full" style={{ backgroundColor: NODE_COLORS.market }} />
              Market
            </span>
            <span className="flex items-center gap-1">
              <span className="w-2 h-2 rounded-full" style={{ backgroundColor: NODE_COLORS.claim }} />
              Claims ({claims.length})
            </span>
            <span className="flex items-center gap-1">
              <span className="w-2 h-2 rounded-full" style={{ backgroundColor: NODE_COLORS.source }} />
              Sources ({sources.length})
            </span>
          </div>
        </div>

        <canvas
          ref={canvasRef}
          className="w-full"
          style={{ height: 400 }}
          onClick={handleCanvasClick}
          onMouseMove={handleMouseMove}
        />

        <div className="p-2 border-t border-border flex items-center gap-4 text-[10px] text-text-muted">
          <span>Analyzed: {new Date(result.metadata.analyzedAt).toLocaleString()}</span>
          <span>{result.metadata.claimCount} claims</span>
          <span>{result.metadata.sourceCount} sources</span>
          <span>{result.metadata.edgeCount} edges</span>
          {result.metadata.snapshotUAL && (
            <Badge variant="accent" className="text-[10px]">DKG: Published</Badge>
          )}
        </div>
      </Card>

      {/* Side panel */}
      <div className="space-y-4 max-h-[600px] overflow-y-auto">
        <MisinfoScoreCard score={result.score} />

        {selectedNode && (
          <Card className="p-4">
            <h3 className="text-sm font-medium text-text-secondary mb-2">
              Selected: {selectedNode.type}
            </h3>
            <pre className="text-xs text-text-primary whitespace-pre-wrap overflow-x-auto">
              {JSON.stringify(selectedNode.data, null, 2)}
            </pre>
          </Card>
        )}

        {claims.length > 0 && (
          <div>
            <h3 className="text-sm font-medium text-text-secondary mb-2">
              Extracted Claims ({claims.length})
            </h3>
            <div className="space-y-2 max-h-60 overflow-y-auto">
              {claims.map((claim) => (
                <ClaimCard key={claim.id} claim={claim} />
              ))}
            </div>
          </div>
        )}

        {sources.length > 0 && (
          <Card className="p-4">
            <h3 className="text-sm font-medium text-text-secondary mb-2">
              Sources ({sources.length})
            </h3>
            <div className="space-y-1 max-h-40 overflow-y-auto">
              {sources.slice(0, 15).map((source) => (
                <SourceCredibilityBar key={source.id} source={source} />
              ))}
            </div>
          </Card>
        )}
      </div>
    </div>
  );
}
