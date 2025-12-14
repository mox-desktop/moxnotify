"use client"

import { useState, useMemo } from "react"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { Label } from "@/components/ui/label"
import { Input } from "@/components/ui/input"
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { X } from "lucide-react"
import type { DbusNotification } from "@/lib/types"

interface FilterSidebarProps {
  notifications: DbusNotification[]
  filters: FilterState
  onFiltersChange: (filters: FilterState) => void
}

export interface FilterState {
  appNames: string[]
  hosts: string[]
  urgency: (0 | 1 | 2) | null
  dateFrom: string
  dateTo: string
}

export function FilterSidebar({ notifications, filters, onFiltersChange }: FilterSidebarProps) {
  // Extract unique values for filters
  const uniqueAppNames = useMemo(() => {
    const names = new Set(notifications.map((n) => n.app_name))
    return Array.from(names).sort()
  }, [notifications])

  const uniqueHosts = useMemo(() => {
    const hosts = new Set(notifications.map((n) => n.host))
    return Array.from(hosts).sort()
  }, [notifications])

  const handleAppNameToggle = (appName: string) => {
    const newAppNames = filters.appNames.includes(appName)
      ? filters.appNames.filter((n) => n !== appName)
      : [...filters.appNames, appName]
    onFiltersChange({ ...filters, appNames: newAppNames })
  }

  const handleHostToggle = (host: string) => {
    const newHosts = filters.hosts.includes(host)
      ? filters.hosts.filter((h) => h !== host)
      : [...filters.hosts, host]
    onFiltersChange({ ...filters, hosts: newHosts })
  }

  const handleUrgencyChange = (value: string) => {
    onFiltersChange({
      ...filters,
      urgency: value === "all" ? null : (Number(value) as 0 | 1 | 2),
    })
  }

  const handleDateFromChange = (value: string) => {
    onFiltersChange({ ...filters, dateFrom: value })
  }

  const handleDateToChange = (value: string) => {
    onFiltersChange({ ...filters, dateTo: value })
  }

  const clearFilters = () => {
    onFiltersChange({
      appNames: [],
      hosts: [],
      urgency: null,
      dateFrom: "",
      dateTo: "",
    })
  }

  const hasActiveFilters =
    filters.appNames.length > 0 ||
    filters.hosts.length > 0 ||
    filters.urgency !== null ||
    filters.dateFrom !== "" ||
    filters.dateTo !== ""

  return (
    <div className="h-screen w-64 border-r bg-muted/30 p-4 overflow-y-auto">
      <div className="mb-4 flex items-center justify-between">
        <div>
          <h2 className="text-lg font-semibold">Indexed Fields</h2>
          <p className="text-xs text-muted-foreground mt-0.5">Filter by indexed fields</p>
        </div>
        {hasActiveFilters && (
          <Button variant="ghost" size="sm" onClick={clearFilters} className="h-7 text-xs">
            Clear
          </Button>
        )}
      </div>

      <div className="space-y-6">
        {/* App Name Filter */}
        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="text-sm font-medium">App Name</CardTitle>
          </CardHeader>
          <CardContent className="space-y-2">
            {uniqueAppNames.length > 0 ? (
              <div className="max-h-48 space-y-1 overflow-y-auto">
                {uniqueAppNames.map((appName) => (
                  <label
                    key={appName}
                    className="flex items-center space-x-2 cursor-pointer rounded-md p-1.5 hover:bg-muted/50"
                  >
                    <input
                      type="checkbox"
                      checked={filters.appNames.includes(appName)}
                      onChange={() => handleAppNameToggle(appName)}
                      className="h-4 w-4 rounded border-gray-300"
                    />
                    <span className="text-sm flex-1 truncate">{appName}</span>
                  </label>
                ))}
              </div>
            ) : (
              <p className="text-xs text-muted-foreground">No app names available</p>
            )}
          </CardContent>
        </Card>

        {/* Host Filter */}
        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="text-sm font-medium">Host</CardTitle>
          </CardHeader>
          <CardContent className="space-y-2">
            {uniqueHosts.length > 0 ? (
              <div className="max-h-48 space-y-1 overflow-y-auto">
                {uniqueHosts.map((host) => (
                  <label
                    key={host}
                    className="flex items-center space-x-2 cursor-pointer rounded-md p-1.5 hover:bg-muted/50"
                  >
                    <input
                      type="checkbox"
                      checked={filters.hosts.includes(host)}
                      onChange={() => handleHostToggle(host)}
                      className="h-4 w-4 rounded border-gray-300"
                    />
                    <span className="text-sm flex-1 truncate">{host}</span>
                  </label>
                ))}
              </div>
            ) : (
              <p className="text-xs text-muted-foreground">No hosts available</p>
            )}
          </CardContent>
        </Card>

        {/* Urgency Filter */}
        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="text-sm font-medium">Urgency</CardTitle>
          </CardHeader>
          <CardContent>
            <Select
              value={filters.urgency === null ? "all" : filters.urgency.toString()}
              onValueChange={handleUrgencyChange}
            >
              <SelectTrigger className="h-8 text-sm">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">All</SelectItem>
                <SelectItem value="0">Low</SelectItem>
                <SelectItem value="1">Normal</SelectItem>
                <SelectItem value="2">Critical</SelectItem>
              </SelectContent>
            </Select>
          </CardContent>
        </Card>

        {/* Date Range Filter */}
        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="text-sm font-medium">Date Range</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="space-y-1.5">
              <Label htmlFor="date-from" className="text-xs">
                From
              </Label>
              <Input
                id="date-from"
                type="datetime-local"
                value={filters.dateFrom}
                onChange={(e) => handleDateFromChange(e.target.value)}
                className="h-8 text-sm"
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="date-to" className="text-xs">
                To
              </Label>
              <Input
                id="date-to"
                type="datetime-local"
                value={filters.dateTo}
                onChange={(e) => handleDateToChange(e.target.value)}
                className="h-8 text-sm"
              />
            </div>
          </CardContent>
        </Card>

        {/* Active Filters Summary */}
        {hasActiveFilters && (
          <Card>
            <CardHeader className="pb-3">
              <CardTitle className="text-sm font-medium">Active Filters</CardTitle>
            </CardHeader>
            <CardContent>
              <div className="flex flex-wrap gap-1.5">
                {filters.appNames.map((appName) => (
                  <Badge key={`app-${appName}`} variant="secondary" className="text-xs">
                    App: {appName}
                    <button
                      onClick={() => handleAppNameToggle(appName)}
                      className="ml-1 hover:text-destructive"
                    >
                      <X className="h-3 w-3" />
                    </button>
                  </Badge>
                ))}
                {filters.hosts.map((host) => (
                  <Badge key={`host-${host}`} variant="secondary" className="text-xs">
                    Host: {host}
                    <button
                      onClick={() => handleHostToggle(host)}
                      className="ml-1 hover:text-destructive"
                    >
                      <X className="h-3 w-3" />
                    </button>
                  </Badge>
                ))}
                {filters.urgency !== null && (
                  <Badge variant="secondary" className="text-xs">
                    Urgency: {filters.urgency === 0 ? "Low" : filters.urgency === 1 ? "Normal" : "Critical"}
                    <button
                      onClick={() => handleUrgencyChange("all")}
                      className="ml-1 hover:text-destructive"
                    >
                      <X className="h-3 w-3" />
                    </button>
                  </Badge>
                )}
                {(filters.dateFrom || filters.dateTo) && (
                  <Badge variant="secondary" className="text-xs">
                    Date Range
                    <button onClick={clearFilters} className="ml-1 hover:text-destructive">
                      <X className="h-3 w-3" />
                    </button>
                  </Badge>
                )}
              </div>
            </CardContent>
          </Card>
        )}
      </div>
    </div>
  )
}

