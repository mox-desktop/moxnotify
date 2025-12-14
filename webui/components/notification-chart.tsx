"use client"

import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { BarChart, Bar, XAxis, YAxis, CartesianGrid, Tooltip, Legend, ResponsiveContainer } from "recharts"
import type { DbusNotification } from "@/lib/types"
import { useMemo } from "react"

interface NotificationChartProps {
  notifications: DbusNotification[]
  timeInterval: number // in minutes
}

export function NotificationChart({ notifications, timeInterval }: NotificationChartProps) {
  const chartData = useMemo(() => {
    const groupedByTime = new Map<number, { Low: number; Normal: number; Critical: number; timestamp: Date }>()

    notifications.forEach((notification) => {
      // Round timestamp to nearest interval
      const time = notification.timestamp.getTime()
      const intervalMs = timeInterval * 60 * 1000
      const roundedTime = Math.floor(time / intervalMs) * intervalMs

      if (!groupedByTime.has(roundedTime)) {
        groupedByTime.set(roundedTime, {
          Low: 0,
          Normal: 0,
          Critical: 0,
          timestamp: new Date(roundedTime),
        })
      }

      const counts = groupedByTime.get(roundedTime)!
      switch (notification.urgency) {
        case 0:
          counts.Low++
          break
        case 1:
          counts.Normal++
          break
        case 2:
          counts.Critical++
          break
      }
    })

    return Array.from(groupedByTime.values())
      .map((data) => ({
        time: data.timestamp.toLocaleTimeString("en-US", {
          hour: "2-digit",
          minute: "2-digit",
          hour12: false,
        }),
        timestamp: data.timestamp.getTime(),
        Low: data.Low,
        Normal: data.Normal,
        Critical: data.Critical,
      }))
      .sort((a, b) => a.timestamp - b.timestamp)
  }, [notifications, timeInterval])

  return (
    <Card className="border-border bg-card">
      <CardHeader>
        <CardTitle className="text-card-foreground">Notification Analytics</CardTitle>
        <CardDescription>Notifications grouped by time and urgency level</CardDescription>
      </CardHeader>
      <CardContent>
        <ResponsiveContainer width="100%" height={350}>
          <BarChart
            data={chartData}
            margin={{
              top: 20,
              right: 30,
              left: 20,
              bottom: 5,
            }}
          >
            <CartesianGrid strokeDasharray="3 3" stroke="#444" opacity={0.5} />
            <XAxis dataKey="time" stroke="#9ca3af" fontSize={12} tick={{ fill: "#9ca3af" }} />
            <YAxis stroke="#9ca3af" fontSize={12} tick={{ fill: "#9ca3af" }} allowDecimals={false} />
            <Tooltip
              contentStyle={{
                backgroundColor: "#1f2937",
                borderColor: "#374151",
                borderRadius: "0.5rem",
                color: "#f3f4f6",
              }}
              labelStyle={{ color: "#f3f4f6" }}
              itemStyle={{ color: "#f3f4f6" }}
            />
            <Legend wrapperStyle={{ color: "#f3f4f6" }} iconType="rect" />
            <Bar dataKey="Low" fill="#10b981" />
            <Bar dataKey="Normal" fill="#3b82f6" />
            <Bar dataKey="Critical" fill="#ef4444" />
          </BarChart>
        </ResponsiveContainer>
      </CardContent>
    </Card>
  )
}
