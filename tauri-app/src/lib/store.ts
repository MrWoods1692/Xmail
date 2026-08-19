import { writable } from 'svelte/store';
import type { EmailAccount, EmailMessage, EmailProviderConfig } from './types';

// 用户信息类型
export interface UserInfo {
  id: string;
  username: string;
  email: string;
  authType?: 'password' | 'oauth2' | 'qq_oauth2';
  avatar?: string; // 用户头像URL
}

// 应用状态
export const accounts = writable<EmailAccount[]>([]);
export const currentAccount = writable<EmailAccount | null>(null);
export const messages = writable<EmailMessage[]>([]);
export const currentMessage = writable<EmailMessage | null>(null);
export const currentFolder = writable<string>('INBOX');
export const isLoading = writable<boolean>(false);
export const error = writable<string | null>(null);

// 用户状态
export const currentUser = writable<UserInfo | null>(null);

// UI状态
export const showAccountDialog = writable<boolean>(false);
export const showComposeDialog = writable<boolean>(false);
export const sidebarCollapsed = writable<boolean>(false);

// 预选的邮箱提供商（用于快速添加）
export const preselectedProvider = writable<EmailProviderConfig | null>(null);

// 通知状态
export const notifications = writable<Array<{
  id: string;
  type: 'success' | 'error' | 'info' | 'warning';
  message: string;
  timestamp: number;
}>>([]);

// 添加通知的辅助函数
export function addNotification(
  type: 'success' | 'error' | 'info' | 'warning',
  message: string
) {
  const id = Math.random().toString(36).substr(2, 9);
  const notification = {
    id,
    type,
    message,
    timestamp: Date.now()
  };
  
  notifications.update(items => [...items, notification]);
  
  // 5秒后自动移除通知
  setTimeout(() => {
    notifications.update(items => items.filter(item => item.id !== id));
  }, 5000);
}

// 清除错误状态
export function clearError() {
  error.set(null);
}

// 设置加载状态
export function setLoading(loading: boolean) {
  isLoading.set(loading);
}
