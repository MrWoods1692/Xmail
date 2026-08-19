export interface EmailAccount {
  id: string;
  name: string;
  email: string;
  imap_server: string;
  imap_port: number;
  smtp_server: string;
  smtp_port: number;
  username: string;
  password: string;
  use_tls: boolean;
  created_at: string;
  updated_at: string;
  // OAuth2 字段（可选）
  auth_type?: string;
  access_token?: string;
  refresh_token?: string;
  token_expires_at?: string;
}

export interface NewEmailAccount {
  name: string;
  email: string;
  imap_server: string;
  imap_port: number;
  smtp_server: string;
  smtp_port: number;
  username: string;
  password: string;
  use_tls: boolean;
  // OAuth2 字段（可选）
  auth_type?: string;
  access_token?: string;
  refresh_token?: string;
  token_expires_at?: string;
}

export interface EmailMessage {
  id: string;
  account_id: string;
  message_id: string;
  subject: string;
  sender: string;
  recipients: string; // JSON string of array
  cc?: string; // JSON string of array
  bcc?: string; // JSON string of array
  body_text?: string;
  body_html?: string;
  folder: string;
  is_read: boolean;
  is_starred: boolean;
  is_deleted: boolean;
  received_at: string;
  created_at: string;
  updated_at: string;
}

export interface EmailAttachment {
  id: string;
  filename: string;
  content_type: string;
  size: number;
  data: number[];
}

export interface SendEmailRequest {
  account_id: string;
  to: string[];
  cc: string[];
  bcc: string[];
  subject: string;
  body_text?: string;
  body_html?: string;
  attachments: EmailAttachment[];
  in_reply_to?: string; // 回复的邮件ID
  references?: string; // 邮件引用链
  draft_id?: string; // 草稿ID（用于更新现有草稿）
}

export interface EmailProviderConfig {
  name: string;
  imap_server: string;
  imap_port: number;
  smtp_server: string;
  smtp_port: number;
  use_tls: boolean;
}

export interface EmailFolder {
  name: string;
  path: string;
  message_count: number;
  unread_count: number;
}

export interface EmailTag {
  text: string;
  tag_type: 'VerificationCode' | 'Important' | 'Folder';
  color: string;
}

// 联系人相关类型
export interface Contact {
  id: string;
  user_id: string;
  name: string;
  email: string;
  phone?: string;
  company?: string;
  notes?: string;
  avatar?: string;
  is_favorite: boolean;
  created_at: string;
  updated_at: string;
}

export interface NewContact {
  user_id: string;
  name: string;
  email: string;
  phone?: string;
  company?: string;
  notes?: string;
  avatar?: string;
  is_favorite: boolean;
}
