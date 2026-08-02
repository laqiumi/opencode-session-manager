export interface SessionInfo {
  source: string;
  id: string;
  title: string;
  directory: string;
  folder_name: string;
  model: string | null;
  time_created: number;
  time_updated: number;
  message_count: number;
  last_user_message: string | null;
}
