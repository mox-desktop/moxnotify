"use client"

import { useState, useEffect, useCallback } from "react"
import { useSearchParams, useRouter, usePathname } from "next/navigation"
import { NotificationChart } from "./notification-chart"
import { NotificationList } from "./notification-list"
import { SearchBar } from "./search-bar"
import type { DbusNotification } from "@/lib/types"
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Label } from "@/components/ui/label"
import { Input } from "@/components/ui/input"
import { Loader2 } from "lucide-react"

const API_BASE_URL = process.env.NEXT_PUBLIC_API_URL || "http://localhost:3029"

export function NotificationDashboard() {
  const searchParams = useSearchParams()
  const router = useRouter()
  const pathname = usePathname()

  // Initialize state from URL parameters
  const [searchInput, setSearchInput] = useState(searchParams.get("q") || "*")
  const [searchQuery, setSearchQuery] = useState(searchParams.get("q") || "*")
  const [notifications, setNotifications] = useState<DbusNotification[]>([])
  const [isLoading, setIsLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [timeInterval, setTimeInterval] = useState(
    Number(searchParams.get("interval")) || 5
  )
  // Time range filter - how far back to show notifications (in minutes, or "all" for no filter)
  const predefinedTimeRanges = [15, 30, 60, 120, 240, 360, 720, 1440, 2880, 4320, 10080, 20160, 43200, 129600, 259200, 525600]
  const initialTimeRange = searchParams.get("timeRange") === "all" 
    ? "all" 
    : searchParams.get("timeRange") 
    ? Number(searchParams.get("timeRange")) 
    : 30
  const [timeRange, setTimeRange] = useState<number | "all">(initialTimeRange)
  const [useCustomTimeRange, setUseCustomTimeRange] = useState(
    typeof initialTimeRange === "number" && !predefinedTimeRanges.includes(initialTimeRange)
  )
  const [customTimeRangeMinutes, setCustomTimeRangeMinutes] = useState(
    typeof initialTimeRange === "number" && !predefinedTimeRanges.includes(initialTimeRange)
      ? initialTimeRange.toString()
      : ""
  )
  const [maxHits, setMaxHits] = useState<number>(
    searchParams.get("maxHits") ? Number(searchParams.get("maxHits")) : 100
  )
  const [sortBy, setSortBy] = useState<string>(searchParams.get("sortBy") || "id")
  const [sortOrder, setSortOrder] = useState<"asc" | "desc">(
    (searchParams.get("sortOrder") as "asc" | "desc") || "desc"
  )
  const [refetchTrigger, setRefetchTrigger] = useState(0)

  // Fetch notifications from API
  const fetchNotifications = useCallback(async () => {
    setIsLoading(true)
    setError(null)

    try {
      const response = await fetch(`${API_BASE_URL}/api/search`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          query: searchQuery || "*",
          max_hits: maxHits,
          sort_by: sortBy,
          sort_order: sortOrder,
        }),
      })

      if (!response.ok) {
        let errorMessage = `API error: ${response.statusText}`
        try {
          const errorData = await response.json()
          if (errorData.error || errorData.message) {
            errorMessage = errorData.error || errorData.message
          }
        } catch {
          // If response is not JSON, use status text
        }
        throw new Error(errorMessage)
      }

      let data
      try {
        data = await response.json()
      } catch (err) {
        throw new Error("Invalid JSON response from API")
      }
      
      // Handle different response formats
      let notificationsArray: any[] = []
      if (Array.isArray(data)) {
        notificationsArray = data
      } else if (data.notifications && Array.isArray(data.notifications)) {
        notificationsArray = data.notifications
      } else if (data.results && Array.isArray(data.results)) {
        notificationsArray = data.results
      } else {
        throw new Error("Unexpected API response format")
      }
      
      // Convert timestamp strings to Date objects if needed
      const notificationsWithDates = notificationsArray.map((n: any) => {
        try {
          // Preserve all fields including expire_timeout (handle different field name variations)
          const expireTimeout = n.expire_timeout !== undefined 
            ? n.expire_timeout 
            : n.expireTimeout !== undefined 
            ? n.expireTimeout 
            : n.timeout !== undefined
            ? n.timeout
            : undefined
          
          // Extract urgency from hints.urgency (always 0, 1, or 2)
          let urgency: 0 | 1 | 2 = 1 // Default to Normal (1)
          if (n.hints) {
            let hintsObj = n.hints
            // Handle hints as array containing JSON string (API format)
            if (Array.isArray(hintsObj) && hintsObj.length > 0 && typeof hintsObj[0] === 'string') {
              try {
                hintsObj = JSON.parse(hintsObj[0])
              } catch (e) {
                console.warn(`Failed to parse hints array[0] as JSON for notification ${n.id}:`, e)
              }
            }
            // If hints is a string, parse it
            else if (typeof hintsObj === 'string') {
              try {
                hintsObj = JSON.parse(hintsObj)
              } catch (e) {
                console.warn(`Failed to parse hints as JSON for notification ${n.id}:`, e)
              }
            }
            
            // Extract urgency from hints object
            if (hintsObj && typeof hintsObj === 'object' && hintsObj !== null && !Array.isArray(hintsObj)) {
              // Check if urgency exists and is a number
              if ('urgency' in hintsObj && typeof hintsObj.urgency === 'number') {
                const urgencyValue = hintsObj.urgency
                if (urgencyValue === 0 || urgencyValue === 1 || urgencyValue === 2) {
                  urgency = urgencyValue as 0 | 1 | 2
                } else {
                  console.warn(`Invalid urgency value ${urgencyValue} for notification ${n.id}`)
                }
              } else {
                console.warn(`No valid urgency found in hints for notification ${n.id}:`, hintsObj)
              }
            }
            
            // Debug: log urgency extraction for first few notifications
            if (notificationsArray.indexOf(n) < 3) {
              console.log(`Notification ${n.id}: hints=`, n.hints, `parsed hints=`, hintsObj, `urgency=`, urgency)
            }
          } else {
            // Debug: log when hints is missing
            if (notificationsArray.indexOf(n) < 3) {
              console.warn(`Notification ${n.id} has no hints field`)
            }
          }
          
          const notification = {
            ...n,
            timestamp: n.timestamp ? new Date(n.timestamp) : new Date(),
            // Explicitly set expire_timeout to ensure it's preserved
            expire_timeout: expireTimeout,
            // Explicitly set urgency to ensure it's preserved and validated
            urgency: urgency,
          }
          
          // Debug: verify urgency is set correctly
          if (notificationsArray.indexOf(n) < 3) {
            console.log(`Final notification ${notification.id}: urgency=${notification.urgency}`)
          }
          
          return notification
        } catch (err) {
          console.error("Error parsing notification:", n, err)
          const expireTimeout = n.expire_timeout !== undefined 
            ? n.expire_timeout 
            : n.expireTimeout !== undefined 
            ? n.expireTimeout 
            : n.timeout
          
          // Extract urgency from hints.urgency
          let urgency: 0 | 1 | 2 = 1 // Default to Normal (1)
          if (n.hints) {
            let hintsObj = n.hints
            // Handle hints as array containing JSON string (API format)
            if (Array.isArray(hintsObj) && hintsObj.length > 0 && typeof hintsObj[0] === 'string') {
              try {
                hintsObj = JSON.parse(hintsObj[0])
              } catch (e) {
                // If parsing fails, use default
              }
            }
            // If hints is a string, parse it
            else if (typeof hintsObj === 'string') {
              try {
                hintsObj = JSON.parse(hintsObj)
              } catch (e) {
                // If parsing fails, use default
              }
            }
            
            if (hintsObj && typeof hintsObj === 'object' && hintsObj !== null && !Array.isArray(hintsObj) && typeof hintsObj.urgency === 'number') {
              const urgencyValue = hintsObj.urgency
              if (urgencyValue === 0 || urgencyValue === 1 || urgencyValue === 2) {
                urgency = urgencyValue as 0 | 1 | 2
              }
            }
          }
          
          return {
            ...n,
            timestamp: new Date(),
            expire_timeout: expireTimeout,
            urgency: urgency,
          }
        }
      })
      
      // Filter notifications by time range if specified
      const now = Date.now()
      const filteredNotifications = timeRange === "all" 
        ? notificationsWithDates
        : notificationsWithDates.filter((n) => {
            const notificationTime = n.timestamp instanceof Date 
              ? n.timestamp.getTime() 
              : new Date(n.timestamp).getTime()
            const timeRangeMs = timeRange * 60 * 1000
            return now - notificationTime <= timeRangeMs
          })
      
      setNotifications(filteredNotifications)
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : "Failed to fetch notifications"
      console.error("Error fetching notifications:", err)
      setError(errorMessage)
      setNotifications([])
    } finally {
      setIsLoading(false)
    }
  }, [searchQuery, maxHits, sortBy, sortOrder, timeRange])

  // Fetch on mount and when search parameters change
  useEffect(() => {
    fetchNotifications()
  }, [fetchNotifications, refetchTrigger])

  // Update URL when state changes
  useEffect(() => {
    const params = new URLSearchParams()

    if (searchQuery.trim() && searchQuery !== "*") {
      params.set("q", searchQuery)
    }

    if (timeInterval !== 5) {
      params.set("interval", timeInterval.toString())
    }

    if (timeRange !== 30 && timeRange !== "all") {
      params.set("timeRange", timeRange.toString())
    } else if (timeRange === "all") {
      params.set("timeRange", "all")
    }

    if (maxHits !== 100) {
      params.set("maxHits", maxHits.toString())
    }

    if (sortBy !== "id") {
      params.set("sortBy", sortBy)
    }

    if (sortOrder !== "desc") {
      params.set("sortOrder", sortOrder)
    }

    const newUrl = params.toString() ? `${pathname}?${params.toString()}` : pathname
    router.replace(newUrl, { scroll: false })
  }, [searchQuery, timeInterval, timeRange, maxHits, sortBy, sortOrder, pathname, router])

  const handleRunSearch = () => {
    const newQuery = searchInput.trim() || "*"
    setSearchQuery(newQuery)
    // Force refetch even if query didn't change by incrementing refetch trigger
    setRefetchTrigger((prev) => prev + 1)
  }

  const handleTimeIntervalChange = (value: string) => {
    setTimeInterval(Number(value))
  }

  const handleTimeRangeChange = (value: string) => {
    if (value === "custom") {
      setUseCustomTimeRange(true)
      if (customTimeRangeMinutes) {
        const minutes = Number(customTimeRangeMinutes)
        if (!isNaN(minutes) && minutes > 0) {
          setTimeRange(minutes)
        }
      }
    } else if (value === "all") {
      setUseCustomTimeRange(false)
      setTimeRange("all")
    } else {
      setUseCustomTimeRange(false)
      setTimeRange(Number(value))
    }
  }

  const handleCustomTimeRangeChange = (value: string) => {
    setCustomTimeRangeMinutes(value)
    const minutes = Number(value)
    if (!isNaN(minutes) && minutes > 0) {
      setTimeRange(minutes)
    }
  }

  const getTimeRangeSelectValue = (): string => {
    if (useCustomTimeRange) {
      return "custom"
    }
    return timeRange === "all" ? "all" : timeRange.toString()
  }

  const handleMaxHitsChange = (value: string) => {
    setMaxHits(Number(value))
  }

  const handleSortByChange = (value: string) => {
    setSortBy(value)
  }

  const handleSortOrderChange = (value: string) => {
    setSortOrder(value as "asc" | "desc")
  }

  return (
    <div className="mx-auto max-w-7xl p-4 md:p-6 lg:p-8">
      <header className="mb-6">
        <h1 className="text-3xl font-bold tracking-tight text-foreground">Notification Center</h1>
        <p className="mt-1 text-sm text-muted-foreground">D-Bus specification compliant notification viewer</p>
      </header>

      <div className="space-y-6">
        <div className="flex flex-wrap items-center gap-3">
          <div className="flex items-center gap-3">
            <Label htmlFor="interval-select" className="text-sm font-medium text-foreground">
              Chart Time Interval:
            </Label>
            <Select value={timeInterval.toString()} onValueChange={handleTimeIntervalChange}>
              <SelectTrigger id="interval-select" className="w-[180px]">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="5">5 minutes</SelectItem>
                <SelectItem value="10">10 minutes</SelectItem>
                <SelectItem value="15">15 minutes</SelectItem>
                <SelectItem value="20">20 minutes</SelectItem>
                <SelectItem value="30">30 minutes</SelectItem>
                <SelectItem value="60">1 hour</SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div className="flex items-center gap-3">
            <Label htmlFor="time-range-select" className="text-sm font-medium text-foreground">
              Display Time Range:
            </Label>
            {useCustomTimeRange ? (
              <div className="flex items-center gap-2">
                <Input
                  id="custom-time-range-input"
                  type="number"
                  min="1"
                  placeholder="Minutes"
                  value={customTimeRangeMinutes}
                  onChange={(e) => handleCustomTimeRangeChange(e.target.value)}
                  className="w-[120px]"
                />
                <Select
                  value={getTimeRangeSelectValue()}
                  onValueChange={handleTimeRangeChange}
                >
                  <SelectTrigger id="time-range-select" className="w-[200px]">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="custom">Custom (minutes)</SelectItem>
                    <SelectItem value="15">Last 15 minutes</SelectItem>
                    <SelectItem value="30">Last 30 minutes</SelectItem>
                    <SelectItem value="60">Last 1 hour</SelectItem>
                    <SelectItem value="120">Last 2 hours</SelectItem>
                    <SelectItem value="240">Last 4 hours</SelectItem>
                    <SelectItem value="360">Last 6 hours</SelectItem>
                    <SelectItem value="720">Last 12 hours</SelectItem>
                    <SelectItem value="1440">Last 1 day</SelectItem>
                    <SelectItem value="2880">Last 2 days</SelectItem>
                    <SelectItem value="4320">Last 3 days</SelectItem>
                    <SelectItem value="10080">Last 1 week</SelectItem>
                    <SelectItem value="20160">Last 2 weeks</SelectItem>
                    <SelectItem value="43200">Last 1 month</SelectItem>
                    <SelectItem value="129600">Last 3 months</SelectItem>
                    <SelectItem value="259200">Last 6 months</SelectItem>
                    <SelectItem value="525600">Last 1 year</SelectItem>
                    <SelectItem value="all">All time</SelectItem>
                  </SelectContent>
                </Select>
              </div>
            ) : (
              <Select
                value={getTimeRangeSelectValue()}
                onValueChange={handleTimeRangeChange}
              >
                <SelectTrigger id="time-range-select" className="w-[200px]">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="15">Last 15 minutes</SelectItem>
                  <SelectItem value="30">Last 30 minutes</SelectItem>
                  <SelectItem value="60">Last 1 hour</SelectItem>
                  <SelectItem value="120">Last 2 hours</SelectItem>
                  <SelectItem value="240">Last 4 hours</SelectItem>
                  <SelectItem value="360">Last 6 hours</SelectItem>
                  <SelectItem value="720">Last 12 hours</SelectItem>
                  <SelectItem value="1440">Last 1 day</SelectItem>
                  <SelectItem value="2880">Last 2 days</SelectItem>
                  <SelectItem value="4320">Last 3 days</SelectItem>
                  <SelectItem value="10080">Last 1 week</SelectItem>
                  <SelectItem value="20160">Last 2 weeks</SelectItem>
                  <SelectItem value="43200">Last 1 month</SelectItem>
                  <SelectItem value="129600">Last 3 months</SelectItem>
                  <SelectItem value="259200">Last 6 months</SelectItem>
                  <SelectItem value="525600">Last 1 year</SelectItem>
                  <SelectItem value="all">All time</SelectItem>
                  <SelectItem value="custom">Custom...</SelectItem>
                </SelectContent>
              </Select>
            )}
          </div>
          <div className="flex items-center gap-3">
            <Label htmlFor="max-hits-select" className="text-sm font-medium text-foreground">
              Max Hits:
            </Label>
            <Select
              value={maxHits.toString()}
              onValueChange={handleMaxHitsChange}
            >
              <SelectTrigger id="max-hits-select" className="w-[180px]">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="10">10</SelectItem>
                <SelectItem value="25">25</SelectItem>
                <SelectItem value="50">50</SelectItem>
                <SelectItem value="100">100</SelectItem>
                <SelectItem value="200">200</SelectItem>
                <SelectItem value="500">500</SelectItem>
                <SelectItem value="1000">1000</SelectItem>
                <SelectItem value="2000">2000</SelectItem>
                <SelectItem value="5000">5000</SelectItem>
                <SelectItem value="10000">10000</SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div className="flex items-center gap-3">
            <Label htmlFor="sort-by-select" className="text-sm font-medium text-foreground">
              Sort By:
            </Label>
            <Select value={sortBy} onValueChange={handleSortByChange}>
              <SelectTrigger id="sort-by-select" className="w-[180px]">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="id">ID</SelectItem>
                <SelectItem value="timestamp">Timestamp</SelectItem>
                <SelectItem value="app_name">App Name</SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div className="flex items-center gap-3">
            <Label htmlFor="sort-order-select" className="text-sm font-medium text-foreground">
              Sort Order:
            </Label>
            <Select value={sortOrder} onValueChange={handleSortOrderChange}>
              <SelectTrigger id="sort-order-select" className="w-[180px]">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="asc">Ascending</SelectItem>
                <SelectItem value="desc">Descending</SelectItem>
              </SelectContent>
            </Select>
          </div>
        </div>

        <NotificationChart notifications={notifications} timeInterval={timeInterval} timeRange={timeRange} />
        <SearchBar value={searchInput} onChange={setSearchInput} onRun={handleRunSearch} />
        
        {isLoading && (
          <div className="flex items-center justify-center py-12">
            <Loader2 className="h-8 w-8 animate-spin text-muted-foreground" />
          </div>
        )}
        
        {error && (
          <div className="rounded-lg border border-destructive bg-destructive/10 p-4">
            <p className="text-sm text-destructive">Error: {error}</p>
          </div>
        )}
        
        {!isLoading && !error && (
          <NotificationList notifications={notifications} />
        )}
      </div>
    </div>
  )
}
