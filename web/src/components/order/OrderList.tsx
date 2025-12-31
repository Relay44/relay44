'use client';

import { Card, Badge, Button, LoadingScreen } from '@/components/ui';
import { useOrders, useCancelOrder, useRuntimeMode } from '@/hooks';
import { formatPrice, formatDateTime } from '@/lib/utils';
import { ORDER_STATUS_LABELS } from '@/lib/constants';
import type { Order } from '@/types';

export interface OrderListProps {
  marketId?: string;
}

export function OrderList({ marketId }: OrderListProps) {
  const { data, isLoading } = useOrders({ marketId, status: 'open' });
  const { readOnly } = useRuntimeMode();

  if (isLoading) {
    return <LoadingScreen />;
  }

  const orders = data?.data || [];

  if (orders.length === 0) {
    return (
      <div className="text-center py-8 text-text-secondary">
        No open orders
      </div>
    );
  }

  return (
    <div className="space-y-3">
      {readOnly ? (
        <div className="border border-border bg-bg-secondary px-3 py-2 text-xs text-text-secondary">
          Order cancellation is disabled in read-only mode.
        </div>
      ) : null}
      {orders.map((order) => (
        <OrderRow key={order.id} order={order} readOnly={readOnly} />
      ))}
    </div>
  );
}

interface OrderRowProps {
  order: Order;
  readOnly: boolean;
}

function OrderRow({ order, readOnly }: OrderRowProps) {
  const cancelOrder = useCancelOrder();

  const statusVariant =
    order.status === 'open'
      ? 'default'
      : order.status === 'partially_filled'
        ? 'warning'
        : 'muted';

  const sideColor = order.side === 'buy' ? 'text-accent' : 'text-text-secondary';
  const outcomeColor = order.outcome === 'yes' ? 'text-bid' : 'text-ask';

  return (
    <Card>
      <div className="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 mb-1">
            <span className={`font-medium ${sideColor}`}>
              {order.side.toUpperCase()}
            </span>
            <span className={outcomeColor}>
              {order.outcome.toUpperCase()}
            </span>
            <Badge variant={statusVariant}>
              {ORDER_STATUS_LABELS[order.status]}
            </Badge>
          </div>

          <div className="grid grid-cols-2 gap-3 text-sm sm:grid-cols-3 sm:gap-4">
            <div>
              <div className="text-text-secondary text-xs">Price</div>
              <div>${formatPrice(order.price)}</div>
            </div>
            <div>
              <div className="text-text-secondary text-xs">Quantity</div>
              <div>
                {order.filledQuantity}/{order.quantity}
              </div>
            </div>
            <div>
              <div className="text-text-secondary text-xs">Created</div>
              <div className="text-xs">{formatDateTime(order.createdAt)}</div>
            </div>
          </div>
        </div>

