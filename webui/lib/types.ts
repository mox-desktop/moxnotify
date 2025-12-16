/**
 * D-Bus Notification Specification
 * Based on org.freedesktop.Notifications
 * https://specifications.freedesktop.org/notification-spec/latest/
 */

export interface DbusNotification {
  // Unique notification ID
  id: number

  // The application name sending the notification
  app_name: string

  // The ID of the notification being replaced (0 if not replacing)
  replaces_id: number

  // The notification icon (freedesktop.org icon naming spec)
  app_icon: string

  // A single line summary of the notification
  summary: string

  // Multi-line body of text
  body: string

  // Actions available for this notification
  actions?: string[]

  // Hints dictionary containing additional data
  hints: Record<string, unknown>

  // The timeout in milliseconds (-1 for default, 0 for never)
  expire_timeout: number

  // Urgency level: 0 (Low), 1 (Normal), 2 (Critical)
  urgency: 0 | 1 | 2

  // Timestamp when the notification was created
  timestamp: Date

  // Host from where the notification was sent
  host: string
}
