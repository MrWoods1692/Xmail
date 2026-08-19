<script lang="ts">
  import { currentMessage, showComposeDialog, currentFolder, currentAccount } from '../store';
  import { EmailAPI } from '../api';
  import { addNotification } from '../store';
  import type { EmailMessage } from '../types';

  let showFullHeaders = false;

  // 检查是否是草稿邮件
  $: isDraft = $currentFolder === 'DRAFTS';

  function formatDate(dateString: string): string {
    const date = new Date(dateString);
    return date.toLocaleString('zh-CN', {
      year: 'numeric',
      month: 'long',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit'
    });
  }

  function formatFileSize(bytes: number): string {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  }

  async function markAsStarred(message: EmailMessage, isStarred: boolean) {
    try {
      if (!$currentAccount) {
        addNotification('error', '未选择账户');
        return;
      }
      await EmailAPI.markAsStarred($currentAccount.id, message.id, isStarred);
      currentMessage.update(msg =>
        msg ? { ...msg, is_starred: isStarred } : msg
      );
      addNotification('success', isStarred ? '已加星标' : '已取消星标');
    } catch (error) {
      addNotification('error', `操作失败: ${error}`);
    }
  }

  function reply() {
    if ($currentMessage) {
      // 这里应该打开回复对话框，预填充收件人和主题
      showComposeDialog.set(true);
      addNotification('info', '回复功能开发中');
    }
  }

  function replyAll() {
    if ($currentMessage) {
      showComposeDialog.set(true);
      addNotification('info', '回复全部功能开发中');
    }
  }

  function forward() {
    if ($currentMessage) {
      showComposeDialog.set(true);
      addNotification('info', '转发功能开发中');
    }
  }

  function deleteMessage() {
    if ($currentMessage) {
      addNotification('info', '删除功能开发中');
    }
  }

  function downloadAttachment(attachment: any) {
    addNotification('info', `下载附件功能开发中: ${attachment.filename}`);
  }

  // 编辑草稿邮件
  function editDraft() {
    if ($currentMessage && isDraft) {
      // 解析草稿邮件内容，提取收件人、主题、正文等信息
      const draftData = parseDraftMessage($currentMessage);

      // 设置写邮件对话框的数据
      // 这里需要一个全局的compose数据状态来预填充
      showComposeDialog.set(true);

      // 触发自定义事件，传递草稿数据
      window.dispatchEvent(new CustomEvent('editDraft', {
        detail: draftData
      }));

      addNotification('info', '正在编辑草稿...');
    }
  }

  // 解析草稿邮件内容
  function parseDraftMessage(message: EmailMessage) {
    // 从邮件内容中提取收件人、抄送、密送、主题、正文
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

  function toggleHeaders() {
    showFullHeaders = !showFullHeaders;
  }

  // 解析JSON字符串为数组
  function parseRecipients(recipients: string): string[] {
    try {
      return JSON.parse(recipients);
    } catch {
      return [recipients];
    }
  }

  // 导入统一的头像工具函数
  import { getSenderDisplayName, getSenderEmail, getSenderAvatarUrl } from '../utils/avatarUtils';

  // 专业的HTML邮件内容清理和格式化
  function cleanHtmlForDisplay(html: string): string {
    let cleanHtml = html;

    // 1. 移除危险的标签和属性（但保留更多格式）
    cleanHtml = cleanHtml
      .replace(/<script[^>]*>[\s\S]*?<\/script>/gi, '')  // 移除script标签
      .replace(/<iframe[^>]*>[\s\S]*?<\/iframe>/gi, '')  // 移除iframe标签
      .replace(/<object[^>]*>[\s\S]*?<\/object>/gi, '')  // 移除object标签
      .replace(/<embed[^>]*>/gi, '')  // 移除embed标签
      .replace(/<form[^>]*>[\s\S]*?<\/form>/gi, '')  // 移除form标签
      .replace(/on\w+\s*=\s*["'][^"']*["']/gi, '')  // 移除事件处理器
      .replace(/javascript:/gi, '');  // 移除javascript协议

    // 2. 保留更多有用的CSS样式（专业邮件客户端风格）
    cleanHtml = cleanHtml.replace(/style\s*=\s*["']([^"']*)["']/gi, (_, styleContent: string) => {
      // 保留更多邮件常用的CSS属性
      const safeStyles = styleContent
        .split(';')
        .filter((style: string) => {
          const prop = style.split(':')[0]?.trim().toLowerCase();
          return prop && [
            'color', 'background-color', 'background', 'font-size', 'font-weight',
            'font-style', 'font-family', 'text-decoration', 'text-align', 'line-height',
            'margin', 'margin-top', 'margin-bottom', 'margin-left', 'margin-right',
            'padding', 'padding-top', 'padding-bottom', 'padding-left', 'padding-right',
            'border', 'border-top', 'border-bottom', 'border-left', 'border-right',
            'border-radius', 'border-color', 'border-width', 'border-style',
            'width', 'max-width', 'height', 'max-height', 'display',
            'vertical-align', 'text-indent', 'letter-spacing', 'word-spacing'
          ].includes(prop);
        })
        .join(';');

      return safeStyles ? `style="${safeStyles}"` : '';
    });

    // 3. 保留基本的CSS样式块（用于邮件格式）
    cleanHtml = cleanHtml.replace(/<style[^>]*>([\s\S]*?)<\/style>/gi, (_, styleContent: string) => {
      // 保留邮件中常用的CSS样式
      const emailStyles = styleContent
        .replace(/(@media[^{]*\{[^{}]*\{[^}]*\}[^}]*\})/gi, '') // 移除复杂的媒体查询
        .replace(/(\.[\w-]+\s*\{[^}]*\})/gi, (match) => {
          // 只保留简单的类选择器样式
          if (match.includes('font') || match.includes('color') || match.includes('text') ||
              match.includes('margin') || match.includes('padding') || match.includes('border')) {
            return match;
          }
          return '';
        });

      return emailStyles.trim() ? `<style>${emailStyles}</style>` : '';
    });

    // 4. 确保图片正确显示
    cleanHtml = cleanHtml.replace(/<img([^>]*)>/gi, (match, attrs) => {
      // 为图片添加响应式样式
      if (!attrs.includes('style=')) {
        return `<img${attrs} style="max-width: 100%; height: auto; display: block;">`;
      }
      return match;
    });

    // 5. 确保链接安全
    cleanHtml = cleanHtml.replace(/<a([^>]*)>/gi, (match, attrs) => {
      // 为链接添加安全属性
      if (!attrs.includes('target=')) {
        return `<a${attrs} target="_blank" rel="noopener noreferrer">`;
      }
      return match;
    });

    return cleanHtml;
  }
</script>

<div class="message-viewer">
  {#if $currentMessage}
    <div class="message-header">
      <div class="message-actions">
        {#if isDraft}
          <!-- 草稿邮件显示编辑按钮 -->
          <button class="action-btn primary" on:click={editDraft} title="编辑草稿">
            ✏️ 编辑草稿
          </button>
        {:else}
          <!-- 普通邮件显示回复、转发等按钮 -->
          <button class="action-btn" on:click={reply} title="回复">
            ↩️ 回复
          </button>
          <button class="action-btn" on:click={replyAll} title="回复全部">
            ↩️ 回复全部
          </button>
          <button class="action-btn" on:click={forward} title="转发">
            ➡️ 转发
          </button>
        {/if}
        <button class="action-btn danger" on:click={deleteMessage} title="删除">
          🗑️ 删除
        </button>
        <button 
          class="action-btn star" 
          class:starred={$currentMessage.is_starred}
          on:click={() => markAsStarred($currentMessage, !$currentMessage.is_starred)}
          title={$currentMessage.is_starred ? '取消星标' : '加星标'}
        >
          {$currentMessage.is_starred ? '⭐' : '☆'}
        </button>
      </div>
    </div>

    <div class="message-content">
      <div class="message-info">
        <h2 class="subject">{$currentMessage.subject || '(无主题)'}</h2>
        
        <div class="message-meta">
          <div class="sender-info">
            <div class="sender-avatar-section">
              <!-- 发件人头像 -->
              <div class="sender-avatar">
                {#await getSenderAvatarUrl($currentMessage.sender)}
                  <div class="avatar-placeholder">
                    {getSenderDisplayName($currentMessage.sender).charAt(0).toUpperCase()}
                  </div>
                {:then avatarUrl}
                  {#if avatarUrl}
                    <img
                      src={avatarUrl}
                      alt={getSenderDisplayName($currentMessage.sender)}
                      class="avatar-image"
                      on:error={() => {
                        // 头像加载失败，缓存已在工具函数中处理
                      }}
                    />
                  {:else}
                    <div class="avatar-placeholder">
                      {getSenderDisplayName($currentMessage.sender).charAt(0).toUpperCase()}
                    </div>
                  {/if}
                {:catch}
                  <div class="avatar-placeholder">
                    {getSenderDisplayName($currentMessage.sender).charAt(0).toUpperCase()}
                  </div>
                {/await}
              </div>

              <div class="sender-details">
                <div class="sender-name">
                  <strong>发件人:</strong> {getSenderDisplayName($currentMessage.sender)}
                </div>
                <div class="sender-email">
                  {getSenderEmail($currentMessage.sender)}
                </div>
              </div>
            </div>

            <div class="message-date">
              {formatDate($currentMessage.received_at)}
            </div>
          </div>

          <div class="recipients">
            <div class="recipient-line">
              <strong>收件人:</strong> {parseRecipients($currentMessage.recipients).join(', ')}
            </div>

            {#if $currentMessage.cc && parseRecipients($currentMessage.cc).length > 0}
              <div class="recipient-line">
                <strong>抄送:</strong> {parseRecipients($currentMessage.cc).join(', ')}
              </div>
            {/if}

            {#if showFullHeaders && $currentMessage.bcc && parseRecipients($currentMessage.bcc).length > 0}
              <div class="recipient-line">
                <strong>密送:</strong> {parseRecipients($currentMessage.bcc).join(', ')}
              </div>
            {/if}
          </div>

          <button class="toggle-headers" on:click={toggleHeaders}>
            {showFullHeaders ? '隐藏详细信息' : '显示详细信息'}
          </button>
        </div>

        <!-- 附件功能暂时隐藏，等待后续实现 -->
        <!-- {#if $currentMessage.attachments && $currentMessage.attachments.length > 0}
          <div class="attachments">
            <h4>附件 ({$currentMessage.attachments.length})</h4>
            <div class="attachment-list">
              {#each $currentMessage.attachments as attachment}
                <div class="attachment-item">
                  <div class="attachment-info">
                    <div class="attachment-name">📎 {attachment.filename}</div>
                    <div class="attachment-size">{formatFileSize(attachment.size)}</div>
                  </div>
                  <button
                    class="download-btn"
                    on:click={() => downloadAttachment(attachment)}
                  >
                    ⬇️ 下载
                  </button>
                </div>
              {/each}
            </div>
          </div>
        {/if} -->
      </div>

      <div class="message-body">
        {#if $currentMessage.body_html && $currentMessage.body_html.trim()}
          <div class="html-content">
            {@html cleanHtmlForDisplay($currentMessage.body_html)}
          </div>
        {:else if $currentMessage.body_text && $currentMessage.body_text.trim()}
          <div class="text-content">
            <pre>{$currentMessage.body_text}</pre>
          </div>
        {:else}
          <div class="no-content">
            <div class="no-content-icon">📭</div>
            <h4>此邮件没有文本内容</h4>
            <p>这封邮件可能只包含附件或其他媒体内容</p>
            <!-- 调试信息 -->
            {#if $currentMessage.body_html !== undefined || $currentMessage.body_text !== undefined}
              <details class="debug-info">
                <summary>调试信息</summary>
                <p>邮件ID: {$currentMessage.id}</p>
                <p>HTML内容: {$currentMessage.body_html ? `存在 (${$currentMessage.body_html.length} 字符)` : '不存在'}</p>
                <p>文本内容: {$currentMessage.body_text ? `存在 (${$currentMessage.body_text.length} 字符)` : '不存在'}</p>
                <p>发件人: {$currentMessage.sender}</p>
                <p>主题: {$currentMessage.subject || '(无主题)'}</p>
              </details>
            {/if}
          </div>
        {/if}
      </div>
    </div>
  {:else}
    <div class="no-message">
      <div class="no-message-icon">📧</div>
      <h3>选择一封邮件来阅读</h3>
      <p>从左侧邮件列表中选择一封邮件来查看其内容</p>
    </div>
  {/if}
</div>

<style>
  .message-viewer {
    flex: 1;
    display: flex;
    flex-direction: column;
    background: white;
    height: 100vh;
    overflow: hidden;
  }

  .message-header {
    padding: 16px 24px;
    border-bottom: 1px solid #e9ecef;
    background: #f8f9fa;
  }

  .message-actions {
    display: flex;
    gap: 8px;
  }

  .action-btn {
    padding: 8px 16px;
    border: 1px solid #dee2e6;
    border-radius: 6px;
    background: white;
    cursor: pointer;
    font-size: 14px;
    transition: all 0.2s;
  }

  .action-btn:hover {
    background: #f8f9fa;
  }

  .action-btn.danger:hover {
    background: #dc3545;
    color: white;
    border-color: #dc3545;
  }

  .action-btn.star.starred {
    color: #ffc107;
  }

  .action-btn.primary {
    background: #007bff;
    color: white;
    border-color: #007bff;
  }

  .action-btn.primary:hover {
    background: #0056b3;
    border-color: #0056b3;
  }

  .message-content {
    flex: 1;
    overflow-y: auto;
    padding: 24px;
  }

  .message-info {
    margin-bottom: 24px;
  }

  .subject {
    margin: 0 0 16px 0;
    color: #495057;
    font-size: 1.5rem;
    line-height: 1.3;
  }

  .message-meta {
    background: #f8f9fa;
    padding: 16px;
    border-radius: 8px;
    margin-bottom: 16px;
  }

  .sender-info {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    margin-bottom: 12px;
  }

  .sender-avatar-section {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .sender-avatar {
    width: 48px;
    height: 48px;
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
    font-size: 18px;
    font-weight: 600;
  }

  .sender-details {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .sender-name {
    color: #495057;
    font-size: 14px;
    font-weight: 600;
  }

  .sender-email {
    color: #6c757d;
    font-size: 13px;
  }

  .message-date {
    color: #6c757d;
    font-size: 14px;
  }

  .recipients {
    margin-bottom: 12px;
  }

  .recipient-line {
    color: #495057;
    font-size: 14px;
    margin-bottom: 4px;
  }

  .toggle-headers {
    background: none;
    border: none;
    color: #007bff;
    cursor: pointer;
    font-size: 14px;
    text-decoration: underline;
  }

  /* 附件相关样式暂时移除，等待后续实现 */

  .message-body {
    border: 1px solid #e9ecef;
    border-radius: 8px;
    overflow: hidden;
  }

  .html-content {
    padding: 0;
    line-height: 1.6;
    word-wrap: break-word;
    overflow-wrap: break-word;
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif;
    font-size: 14px;
    color: #333;
    background: transparent;
  }

  .text-content {
    padding: 20px;
  }

  .text-content pre {
    white-space: pre-wrap;
    word-wrap: break-word;
    font-family: inherit;
    margin: 0;
    line-height: 1.6;
  }

  .no-content {
    padding: 40px;
    text-align: center;
    color: #6c757d;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
  }

  .no-content-icon {
    font-size: 3rem;
    margin-bottom: 16px;
  }

  .no-content h4 {
    margin: 0 0 8px 0;
    font-size: 1.1rem;
    color: #495057;
  }

  .no-content p {
    margin: 0 0 16px 0;
    font-size: 0.9rem;
  }

  .debug-info {
    margin-top: 16px;
    text-align: left;
    font-size: 0.8rem;
    color: #6c757d;
    background: #f8f9fa;
    padding: 12px;
    border-radius: 4px;
    border: 1px solid #e9ecef;
    max-width: 400px;
  }

  .debug-info summary {
    cursor: pointer;
    font-weight: 600;
    margin-bottom: 8px;
  }

  .debug-info p {
    margin: 4px 0;
    font-family: monospace;
    word-break: break-all;
  }

  .no-message {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: #6c757d;
    text-align: center;
  }

  .no-message-icon {
    font-size: 4rem;
    margin-bottom: 20px;
  }

  .no-message h3 {
    margin: 0 0 10px 0;
    font-size: 1.3rem;
  }

  .no-message p {
    margin: 0;
    font-size: 1rem;
  }

  /* 专业邮件客户端样式 - 最小干预原则 */
  .html-content :global(*) {
    max-width: 100%;
    box-sizing: border-box;
  }

  /* 保留邮件原有的段落样式，只做必要调整 */
  .html-content :global(p) {
    line-height: 1.6;
    margin: 1em 0;
  }

  /* 表格样式 - 保持邮件原有样式 */
  .html-content :global(table) {
    border-collapse: collapse;
    margin: 12px 0;
  }

  .html-content :global(td),
  .html-content :global(th) {
    vertical-align: top;
  }

  /* 图片响应式处理 */
  .html-content :global(img) {
    max-width: 100%;
    height: auto;
  }

  /* 链接样式 */
  .html-content :global(a) {
    color: #007bff;
    text-decoration: none;
  }

  .html-content :global(a:hover) {
    text-decoration: underline;
  }

  /* 标题样式 */
  .html-content :global(h1),
  .html-content :global(h2),
  .html-content :global(h3),
  .html-content :global(h4),
  .html-content :global(h5),
  .html-content :global(h6) {
    margin: 1.5em 0 0.5em 0;
    line-height: 1.3;
  }

  /* 列表样式 */
  .html-content :global(ul),
  .html-content :global(ol) {
    margin: 1em 0;
    padding-left: 2em;
  }

  /* 引用样式 */
  .html-content :global(blockquote) {
    margin: 1em 0;
    padding: 0.5em 1em;
    border-left: 4px solid #ddd;
    background: #f9f9f9;
  }

  /* 代码样式 */
  .html-content :global(code) {
    background: #f5f5f5;
    padding: 2px 4px;
    border-radius: 3px;
    font-family: 'Courier New', monospace;
  }

  .html-content :global(pre) {
    background: #f5f5f5;
    padding: 1em;
    border-radius: 5px;
    overflow-x: auto;
    white-space: pre-wrap;
  }
</style>
