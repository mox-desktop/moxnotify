"use client"

import { Card, CardContent } from "@/components/ui/card"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import type { DbusNotification } from "@/lib/types"
import { Bell, AlertTriangle, Info, ImageIcon } from "lucide-react"
import { formatDistanceToNow } from "date-fns"

interface NotificationListProps {
  notifications: DbusNotification[]
}

export function NotificationList({ notifications }: NotificationListProps) {
  if (notifications.length === 0) {
    return (
      <Card className="border-dashed">
        <CardContent className="flex flex-col items-center justify-center py-12">
          <Bell className="mb-2 h-12 w-12 text-muted-foreground" />
          <p className="text-sm text-muted-foreground">No notifications found</p>
        </CardContent>
      </Card>
    )
  }

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <h2 className="text-xl font-semibold tracking-tight">
          Notifications
          <span className="ml-2 text-sm font-normal text-muted-foreground">({notifications.length})</span>
        </h2>
      </div>

      <div className="space-y-1.5">
        {notifications.map((notification, index) => (
          <NotificationCard key={`${notification.id}-${notification.timestamp.getTime()}-${index}`} notification={notification} />
        ))}
      </div>
    </div>
  )
}

function NotificationCard({ notification }: { notification: DbusNotification }) {
  const urgencyConfig = {
    0: {
      label: "Low",
      icon: Info,
      color: "bg-chart-2 text-background",
      borderColor: "border-l-chart-2",
    },
    1: {
      label: "Normal",
      icon: Bell,
      color: "bg-chart-1 text-primary-foreground",
      borderColor: "border-l-chart-1",
    },
    2: {
      label: "Critical",
      icon: AlertTriangle,
      color: "bg-destructive text-destructive-foreground",
      borderColor: "border-l-destructive",
    },
  }

  // Default to Normal (1) if urgency is invalid or undefined
  const urgency = notification.urgency
  const validUrgency = urgency === 0 || urgency === 1 || urgency === 2 ? urgency : 1
  const config = urgencyConfig[validUrgency]
  const Icon = config.icon
  
  const actionButtons: Array<{ key: string; label: string }> = []
  if (notification.actions && Array.isArray(notification.actions)) {
    for (let i = 0; i < notification.actions.length; i += 2) {
      if (i + 1 < notification.actions.length) {
        actionButtons.push({
          key: notification.actions[i],
          label: notification.actions[i + 1],
        })
      }
    }
  }

  return (
    <Card className={`border-l-4 ${config.borderColor} transition-colors hover:bg-muted/50`}>
      <CardContent className="p-2.5">
        <div className="flex gap-3">
          {notification.app_icon ? (
            <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg border bg-muted">
              <ImageIcon className="h-5 w-5 text-muted-foreground" />
              <span className="sr-only">{notification.app_icon}</span>
            </div>
          ) : (
            <div className={`flex h-10 w-10 shrink-0 items-center justify-center rounded-lg ${config.color}`}>
              <Icon className="h-5 w-5" />
            </div>
          )}

          <div className="min-w-0 flex-1 space-y-2">
            <div className="flex items-start justify-between gap-2">
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <h3 className="font-semibold text-card-foreground">{notification.summary}</h3>
                  <Badge variant="outline" className="text-xs">
                    {config.label}
                  </Badge>
                </div>
                <p className="mt-0.5 text-xs text-muted-foreground">
                  {notification.app_name}
                  <span className="ml-2 text-muted-foreground/70">• {notification.host}</span>
                </p>
              </div>
              <time className="shrink-0 text-xs text-muted-foreground">
                {formatDistanceToNow(notification.timestamp, { addSuffix: true })}
              </time>
            </div>

            {notification.body && <p className="text-sm text-muted-foreground">{notification.body}</p>}

            {notification.app_icon && (
              <div className="flex items-center gap-1.5 text-xs text-muted-foreground">
                <ImageIcon className="h-3 w-3" />
                <span className="font-mono">{notification.app_icon}</span>
              </div>
            )}

            {actionButtons.length > 0 && (
              <div className="flex flex-wrap gap-2">
                {actionButtons.map((action) => (
                  <Button
                    key={action.key}
                    variant={action.key === "default" ? "default" : "outline"}
                    size="sm"
                    className="h-7 text-xs"
                    onClick={() => console.log(`Action triggered: ${action.key}`)}
                  >
                    {action.label}
                  </Button>
                ))}
              </div>
            )}

            <div className="flex flex-wrap gap-2 pt-1">
              <span className="font-mono text-xs text-muted-foreground">ID: {notification.id}</span>
              {(() => {
                // Debug: log first notification to see expire_timeout
                if (notification.id === 1 || Object.keys(notification).includes('expire_timeout')) {
                  console.log("Notification expire_timeout value:", notification.expire_timeout, "Type:", typeof notification.expire_timeout, "Full keys:", Object.keys(notification))
                }
                // Always try to display timeout - check multiple possible field names
                const timeout = notification.expire_timeout !== undefined 
                  ? notification.expire_timeout 
                  : (notification as any).expireTimeout !== undefined
                  ? (notification as any).expireTimeout
                  : (notification as any).timeout
                
                return timeout !== undefined && timeout !== null && (
                  <span className="font-mono text-xs text-muted-foreground">
                    {timeout === -1
                      ? "Timeout: Default"
                      : timeout === 0
                      ? "Timeout: Never"
                      : `Timeout: ${timeout}ms`}
                  </span>
                )
              })()}
            </div>
          </div>
        </div>
      </CardContent>
    </Card>
  )
}
