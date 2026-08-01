export interface SessionInfo {
  id: string;
  title: string;
  directory: string;
  folder_name: string;
  project_name: string | null;
  model: string | null;
  agent: string | null;
  time_created: number;
  time_updated: number;
  time_archived: number | null;
  message_count: number;
  cost: number;
  tokens_input: number;
  tokens_output: number;
  last_user_message: string | null;
}
