<script lang="ts">
  import { messages, currentMessage, currentAccount, currentFolder, showComposeDialog } from '../store';
  import { EmailAPI } from '../api';
  import { addNotification } from '../store';
  import type { EmailMessage } from '../types';

  export let selectedMessages: Set<string> = new Set();

  function formatDate(dateString: string): string {
    const date = new Date(dateString);
    const now = new Date();
    const diffTime = Math.abs(now.getTime() - date.getTime());
    const diffDays = Math.ceil(diffTime / (1000 * 60 * 60 * 24));

    if (diffDays === 1) {
      return date.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' });
    } else if (diffDays <= 7) {
      return date.toLocaleDateString('zh-CN', { weekday: 'short' });
    } else {
      return date.toLocaleDateString('zh-CN', { month: 'short', day: 'numeric' });
    }
  }

  function truncateText(text: string, maxLength: number): string {
    if (text.length <= maxLength) return text;
    return text.substring(0, maxLength) + '...';
  }

  function selectMessage(message: EmailMessage) {
    console.log('🔥🔥🔥 selectMessage called, currentFolder:', $currentFolder, 'message:', message.subject);
    alert('selectMessage被调用了！当前文件夹：' + $currentFolder);

    // 如果是草稿文件夹，直接编辑草稿
    if ($currentFolder === 'DRAFTS') {
      console.log('🔥 草稿文件夹，直接编辑草稿');
      editDraftMessage(message);
      return; // 草稿模式不设置currentMessage
    } else {
      console.log('🔥 普通文件夹，显示详情');
      // 普通邮件，显示详情
      currentMessage.set(message);
      if (!message.is_read) {
        markAsRead(message.id, true);
      }
    }
  }

  // 编辑草稿邮件
  function editDraftMessage(message: EmailMessage) {
    console.log('🔥 editDraftMessage called:', message);
    alert('编辑草稿功能被调用！');
    // 解析草稿邮件内容
    const draftData = parseDraftMessage(message);

    // 打开写邮件对话框
    showComposeDialog.set(true);

    // 触发自定义事件，传递草稿数据
    window.dispatchEvent(new CustomEvent('editDraft', {
      detail: draftData
    }));
  }

  // 解析草稿邮件内容
  function parseDraftMessage(message: EmailMessage) {
    let to: string[] = [];
    let cc: string[] = [];
    let bcc: string[] = [];

    try {
      // recipients是JSON字符串，需要解析
      if (message.recipients) {
        to = JSON.parse(message.recipients);
      }
      if (message.cc) {
        cc = JSON.parse(message.cc);
      }
      if (message.bcc) {
        bcc = JSON.parse(message.bcc);
      }
    } catch (e) {
      console.warn('解析草稿邮件收件人信息失败:', e);
      // 如果JSON解析失败，尝试按逗号分割
      to = message.recipients ? message.recipients.split(',').map((email: string) => email.trim()) : [];
      cc = message.cc ? message.cc.split(',').map((email: string) => email.trim()) : [];
      bcc = message.bcc ? message.bcc.split(',').map((email: string) => email.trim()) : [];
    }

    return {
      to,
      cc,
      bcc,
      subject: message.subject || '',
      body: message.body_text || message.body_html || '',
      isDraft: true,
      draftId: message.id
    };
  }

  async function markAsRead(messageId: string, isRead: boolean) {
    try {
      if (!$currentAccount) {
        addNotification('error', '未选择账户');
        return;
      }
      await EmailAPI.markAsRead($currentAccount.id, messageId, isRead);
      messages.update(msgs =>
        msgs.map(msg =>
          msg.id === messageId ? { ...msg, is_read: isRead } : msg
        )
      );
    } catch (error) {
      addNotification('error', `标记邮件失败: ${error}`);
    }
  }

  async function markAsStarred(messageId: string, isStarred: boolean) {
    try {
      if (!$currentAccount) {
        addNotification('error', '未选择账户');
        return;
      }
      await EmailAPI.markAsStarred($currentAccount.id, messageId, isStarred);
      messages.update(msgs =>
        msgs.map(msg =>
          msg.id === messageId ? { ...msg, is_starred: isStarred } : msg
        )
      );
      addNotification('success', isStarred ? '已加星标' : '已取消星标');
    } catch (error) {
      addNotification('error', `操作失败: ${error}`);
    }
  }

  function toggleMessageSelection(messageId: string) {
    if (selectedMessages.has(messageId)) {
      selectedMessages.delete(messageId);
    } else {
      selectedMessages.add(messageId);
    }
    selectedMessages = selectedMessages; // 触发响应式更新
  }

  function openCompose() {
    showComposeDialog.set(true);
  }

  function getPreviewText(message: EmailMessage): string {
    // 优先使用纯文本内容
    if (message.body_text && message.body_text.trim()) {
      return truncateText(message.body_text.trim(), 100);
    }

    // 如果没有纯文本，从HTML中提取
    if (message.body_html && message.body_html.trim()) {
      const htmlText = message.body_html
        .replace(/<br\s*\/?>/gi, '\n')
        .replace(/<\/p>/gi, '\n')
        .replace(/<\/div>/gi, '\n')
        .replace(/<[^>]*>/g, '')
        .replace(/&nbsp;/g, ' ')
        .replace(/&amp;/g, '&')
        .replace(/&lt;/g, '<')
        .replace(/&gt;/g, '>')
        .replace(/&quot;/g, '"')
        .trim();

      if (htmlText) {
        return truncateText(htmlText, 100);
      }
    }

    // 如果都没有内容，显示提示
    return '(此邮件没有文本内容)';
  }

  // 导入统一的头像工具函数
  import { getSenderDisplayName, getSenderAvatarUrl } from '../utils/avatarUtils';
</script>

<div class="message-list">
  <div class="message-list-header">
    <div class="header-left">
      <h3>
        {#if $currentFolder === 'INBOX'}
          📥 收件箱
        {:else if $currentFolder === 'Sent'}
          📤 已发送
        {:else if $currentFolder === 'DRAFTS'}
          📝 草稿箱
        {:else if $currentFolder === 'Trash'}
          🗑️ 垃圾箱
        {:else if $currentFolder === 'Starred'}
          ⭐ 已加星标
        {:else}
          📁 {$currentFolder}
        {/if}
      </h3>
      <span class="message-count">({$messages.length} 封邮件)</span>
    </div>
    
    <div class="header-actions">
      <button class="compose-btn" on:click={openCompose}>
        ✏️ 写邮件
      </button>
      <button class="refresh-btn" on:click={() => addNotification('info', '刷新功能开发中')}>
        🔄 刷新
      </button>
    </div>
  </div>

  <div class="message-list-content">
    {#if $messages.length === 0}
      <div class="empty-state">
        <div class="empty-icon">📭</div>
        <h4>暂无邮件</h4>
        <p>此文件夹中没有邮件</p>
      </div>
    {:else}
      {#each $messages as message}
        <div 
          class="message-item"
          class:unread={!message.is_read}
          class:selected={$currentMessage?.id === message.id}
          on:click={() => selectMessage(message)}
        >
          <div class="message-checkbox">
            <input 
              type="checkbox" 
              checked={selectedMessages.has(message.id)}
              on:change={() => toggleMessageSelection(message.id)}
              on:click|stopPropagation
            />
          </div>

          <!-- 发件人头像 -->
          <div class="message-avatar">
            {#await getSenderAvatarUrl(message.sender)}
              <div class="avatar-placeholder">
                {getSenderDisplayName(message.sender).charAt(0).toUpperCase()}
              </div>
            {:then avatarUrl}
              {#if avatarUrl}
                <img
                  src={avatarUrl}
                  alt={getSenderDisplayName(message.sender)}
                  class="avatar-image"
                  on:error={() => {
                    // 头像加载失败，缓存已在工具函数中处理
                  }}
                />
              {:else}
                <div class="avatar-placeholder">
                  {getSenderDisplayName(message.sender).charAt(0).toUpperCase()}
                </div>
              {/if}
            {:catch}
              <div class="avatar-placeholder">
                {getSenderDisplayName(message.sender).charAt(0).toUpperCase()}
              </div>
            {/await}
          </div>

          <div class="message-star" on:click|stopPropagation={() => markAsStarred(message.id, !message.is_starred)}>
            {message.is_starred ? '⭐' : '☆'}
          </div>

          <div class="message-content">
            <div class="message-header">
              <div class="message-from">{getSenderDisplayName(message.sender)}</div>
              <div class="message-date">{formatDate(message.received_at)}</div>
            </div>
            
            <div class="message-subject">
              {message.subject || '(无主题)'}
              <!-- 附件功能暂时隐藏 -->
              <!-- {#if message.attachments && message.attachments.length > 0}
                <span class="attachment-icon">📎</span>
              {/if} -->
            </div>
            
            <div class="message-preview">
              {getPreviewText(message)}
            </div>
          </div>
        </div>
      {/each}
    {/if}
  </div>
</div>

<style>
  .message-list {
    flex: 1;
    display: flex;
    flex-direction: column;
    background: white;
    border-right: 1px solid #e9ecef;
  }

  .message-list-header {
    padding: 20px;
    border-bottom: 1px solid #e9ecef;
    display: flex;
    justify-content: space-between;
    align-items: center;
    background: #f8f9fa;
  }

  .header-left {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .header-left h3 {
    margin: 0;
    color: #495057;
    font-size: 1.2rem;
  }

  .message-count {
    color: #6c757d;
    font-size: 0.9rem;
  }

  .header-actions {
    display: flex;
    gap: 10px;
  }

  .compose-btn, .refresh-btn {
    padding: 8px 16px;
    border: 1px solid #dee2e6;
    border-radius: 6px;
    background: white;
    cursor: pointer;
    font-size: 14px;
    transition: all 0.2s;
  }

  .compose-btn {
    background: #007bff;
    color: white;
    border-color: #007bff;
  }

  .compose-btn:hover {
    background: #0056b3;
  }

  .refresh-btn:hover {
    background: #f8f9fa;
  }

  .message-list-content {
    flex: 1;
    overflow-y: auto;
  }

  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: #6c757d;
    text-align: center;
  }

  .empty-icon {
    font-size: 4rem;
    margin-bottom: 20px;
  }

  .empty-state h4 {
    margin: 0 0 10px 0;
    font-size: 1.2rem;
  }

  .empty-state p {
    margin: 0;
    font-size: 0.9rem;
  }

  .message-item {
    display: flex;
    align-items: flex-start;
    padding: 16px 20px;
    border-bottom: 1px solid #f1f3f4;
    cursor: pointer;
    transition: background-color 0.2s;
  }

  .message-item:hover {
    background: #f8f9fa;
  }

  .message-item.selected {
    background: #e3f2fd;
    border-left: 3px solid #007bff;
  }

  .message-item.unread {
    background: #fff;
    font-weight: 600;
  }

  .message-item.unread .message-subject {
    font-weight: 700;
  }

  .message-checkbox {
    margin-right: 12px;
    margin-top: 2px;
  }

  .message-avatar {
    margin-right: 12px;
    margin-top: 2px;
    width: 32px;
    height: 32px;
    border-radius: 50%;
    overflow: hidden;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .avatar-image {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .avatar-placeholder {
    width: 100%;
    height: 100%;
    background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
    color: white;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 14px;
    font-weight: 600;
  }

  .message-star {
    margin-right: 12px;
    margin-top: 2px;
    cursor: pointer;
    font-size: 16px;
    color: #ffc107;
  }

  .message-content {
    flex: 1;
    min-width: 0;
  }

  .message-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 4px;
  }

  .message-from {
    font-weight: 600;
    color: #495057;
    font-size: 14px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 200px;
  }

  .message-date {
    color: #6c757d;
    font-size: 12px;
    white-space: nowrap;
  }

  .message-subject {
    color: #495057;
    font-size: 14px;
    margin-bottom: 4px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .attachment-icon {
    color: #6c757d;
    font-size: 12px;
  }

  .message-preview {
    color: #6c757d;
    font-size: 13px;
    line-height: 1.4;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
