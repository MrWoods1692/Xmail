import { invoke } from "@tauri-apps/api/core";
import type { 
  EmailAccount, 
  NewEmailAccount, 
  EmailMessage, 
  SendEmailRequest, 
  EmailProviderConfig 
} from "./types";

export class EmailAPI {
  static async initDatabase(): Promise<void> {
    await invoke("init_database");
  }

  static async createAccount(account: NewEmailAccount): Promise<EmailAccount> {
    return await invoke("create_email_account", { account });
  }

  static async getAccounts(): Promise<EmailAccount[]> {
    return await invoke("get_email_accounts");
  }

  static async deleteAccount(accountId: string): Promise<void> {
    await invoke("delete_email_account", { accountId });
  }

  static async testConnection(accountId: string): Promise<boolean> {
    return await invoke("test_account_connection", { accountId });
  }

  static async testEmailConnection(account: NewEmailAccount): Promise<boolean> {
    return await invoke("test_email_connection", { account });
  }

  static async getMessages(
    accountId: string,
    folder?: string,
    limit?: number,
    forceRefresh?: boolean
  ): Promise<EmailMessage[]> {
    // 调试信息：记录API调用
    console.log(`[API调试] EmailAPI.getMessages 调用 - 账户: ${accountId}, 文件夹: ${folder}, 限制: ${limit}, 强制刷新: ${forceRefresh}`);
    return await invoke("get_messages", { accountId, folder, limit, forceRefresh });
  }

  static async refreshMessages(
    accountId: string,
    folder?: string
  ): Promise<EmailMessage[]> {
    return await invoke("refresh_messages", { accountId, folder });
  }

  static async clearCache(accountId?: string): Promise<void> {
    return await invoke("clear_cache", { accountId });
  }

  static async getCacheStats(): Promise<Record<string, number>> {
    return await invoke("get_cache_stats");
  }

  static async deleteMessage(
    accountId: string,
    messageId: string,
    currentFolder?: string
  ): Promise<boolean> {
    return await invoke("delete_message", { accountId, messageId, currentFolder });
  }

  static async moveMessage(
    accountId: string,
    messageId: string,
    fromFolder: string,
    toFolder: string
  ): Promise<boolean> {
    return await invoke("move_message", { accountId, messageId, fromFolder, toFolder });
  }

  static async analyzeEmailTags(
    subject: string,
    body: string
  ): Promise<EmailTag[]> {
    return await invoke("analyze_email_tags", { subject, body });
  }

  static async sendEmail(
    accountId: string,
    request: SendEmailRequest
  ): Promise<void> {
    await invoke("send_email", { accountId, request });
  }

  static async saveDraft(
    accountId: string,
    request: SendEmailRequest
  ): Promise<void> {
    await invoke("save_draft", { accountId, request });
  }

  static async markAsRead(accountId: string, messageId: string, isRead: boolean): Promise<void> {
    await invoke("mark_message_as_read", { accountId, messageId, isRead });
  }

  static async markAsStarred(accountId: string, messageId: string, isStarred: boolean): Promise<void> {
    await invoke("mark_message_as_starred", { accountId, messageId, isStarred });
  }

  static async getStarredMessages(accountId: string): Promise<EmailMessage[]> {
    return await invoke("get_starred_messages", { accountId });
  }

  static async getEmailProviders(): Promise<EmailProviderConfig[]> {
    return await invoke("get_email_providers");
  }

  static async clearEmailCache(): Promise<string> {
    return await invoke("clear_email_cache");
  }

  // 自动同步相关方法
  static async startAutoSync(intervalMinutes?: number): Promise<string> {
    return await invoke("start_auto_sync", { intervalMinutes });
  }

  static async stopAutoSync(): Promise<string> {
    return await invoke("stop_auto_sync");
  }

  static async getAutoSyncStatus(): Promise<{ enabled: boolean; interval_minutes: number }> {
    return await invoke("get_auto_sync_status");
  }

  static async autoInitializeAllAccounts(): Promise<string> {
    return await invoke("auto_initialize_all_accounts");
  }

  static async notifyInitialLoadComplete(): Promise<string> {
    return await invoke("notify_initial_load_complete");
  }

  static async checkNewMessages(accountId: string): Promise<number> {
    return await invoke("check_new_messages", { accountId });
  }

  // Outlook阻止发件人功能
  static async blockSender(accountId: string, senderEmail: string): Promise<boolean> {
    return await invoke("block_sender", { accountId, senderEmail });
  }

  static async unblockSender(accountId: string, senderEmail: string): Promise<boolean> {
    return await invoke("unblock_sender", { accountId, senderEmail });
  }

  static async isSenderBlocked(accountId: string, senderEmail: string): Promise<boolean> {
    return await invoke("is_sender_blocked", { accountId, senderEmail });
  }

  // 联系人管理
  static async createContact(contactData: any): Promise<any> {
    return await invoke('create_contact', { contactData });
  }

  static async getContacts(userId: string): Promise<any[]> {
    return await invoke('get_contacts', { userId });
  }

  static async updateContact(contactId: string, contactData: any): Promise<string> {
    return await invoke('update_contact', { contactId, contactData });
  }

  static async deleteContact(contactId: string, userId: string): Promise<string> {
    return await invoke('delete_contact', { contactId, userId });
  }

}
