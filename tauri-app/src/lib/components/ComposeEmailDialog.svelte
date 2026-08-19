<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { currentAccount, addNotification, accounts } from '../store';
  import { EmailAPI } from '../api';
  import type { EmailMessage } from '../types';

  const dispatch = createEventDispatcher();

  export let show = false;
  export let initialTo = '';
  export let initialSubject = '';
  export let initialBody = '';
  export let isForwardMode = false; // 是否为转发模式
  export let forwardEmail: EmailMessage | null = null; // 转发的原始邮件对象

  let to = '';
  let cc = '';
  let bcc = '';
  let subject = '';
  let body = '';
  let isSending = false;
  let isDraftMode = false;
  let draftId = '';
  let selectedSenderAccountId = '';

  // 验证状态
  let toValid = true;
  let ccValid = true;
  let bccValid = true;

  // 富文本编辑器
  let editorElement: HTMLDivElement;
  let showColorPicker = false;
  let showFontSizePicker = false;

  // 监听草稿编辑事件
  function handleEditDraft(event: Event) {
    const customEvent = event as CustomEvent;
    const draftData = customEvent.detail;

    // 预填充表单数据
    to = draftData.to.join(', ');
    cc = draftData.cc.join(', ');
    bcc = draftData.bcc.join(', ');
    subject = draftData.subject;
    body = draftData.body;
    isDraftMode = draftData.isDraft;
    draftId = draftData.draftId;
  }

  // 监听联系人邮件事件
  function handleComposeToContact(event: Event) {
    const customEvent = event as CustomEvent;
    const contactData = customEvent.detail;

    // 预填充收件人
    to = contactData.to;
    cc = '';
    bcc = '';
    subject = '';
    body = '';
    if (editorElement) {
      editorElement.innerHTML = '';
    }
    isDraftMode = false;
    draftId = '';
  }

  // 在组件挂载时添加事件监听器
  import { onMount, onDestroy } from 'svelte';

  onMount(() => {
    window.addEventListener('editDraft', handleEditDraft);
    window.addEventListener('composeToContact', handleComposeToContact);
  });

  // 响应式更新发件人选择
  $: if (show && $currentAccount && !selectedSenderAccountId) {
    selectedSenderAccountId = $currentAccount.id;
  }

  onDestroy(() => {
    window.removeEventListener('editDraft', handleEditDraft);
    window.removeEventListener('composeToContact', handleComposeToContact);
  });

  function closeDialog() {
    show = false;
    // 清空表单
    to = '';
    cc = '';
    bcc = '';
    subject = '';
    body = '';
    if (editorElement) {
      editorElement.innerHTML = '';
    }
    isDraftMode = false;
    draftId = '';
    selectedSenderAccountId = $currentAccount?.id || '';
    hasSetInitialValues = false; // 重置初始值标志
    dispatch('close');
  }

  // 验证邮箱地址格式
  function validateEmail(email: string): boolean {
    const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
    return emailRegex.test(email);
  }

  // 验证邮箱列表
  function validateEmailList(emailList: string[], fieldName: string): boolean {
    for (const email of emailList) {
      if (!validateEmail(email)) {
        addNotification('error', `${fieldName}中的邮箱地址格式不正确: ${email}`);
        return false;
      }
    }
    return true;
  }

  // 实时验证邮箱输入
  function validateEmailInput(value: string): boolean {
    if (!value.trim()) return true; // 空值认为有效
    const emails = value.split(',').map(email => email.trim()).filter(email => email);
    return emails.every(email => validateEmail(email));
  }

  // 响应式验证
  $: toValid = validateEmailInput(to);
  $: ccValid = validateEmailInput(cc);
  $: bccValid = validateEmailInput(bcc);

  // 设置初始值
  let hasSetInitialValues = false;

  $: if (show && !hasSetInitialValues) {
    if (initialTo) {
      to = initialTo;
    }
    if (initialSubject) {
      subject = initialSubject;
    }
    if (initialBody) {
      body = initialBody;
      // 延迟设置编辑器内容，确保DOM已渲染
      setTimeout(() => {
        if (editorElement) {
          setEditorContent(initialBody);

          // 如果是转发模式，清空编辑器，显示占位符
          if (isForwardMode) {
            // 完全清空编辑器，让占位符显示
            editorElement.innerHTML = '';

            // 主动移除焦点，让用户点击时才聚焦
            editorElement.blur();
          }
        }
      }, 100);
    }
    hasSetInitialValues = true;
  }

  // 重置标志当对话框关闭时
  $: if (!show) {
    hasSetInitialValues = false;
  }

  // 富文本编辑器功能
  function execCommand(command: string, value?: string) {
    document.execCommand(command, false, value);
    editorElement?.focus();
  }

  function formatText(command: string) {
    execCommand(command);
  }

  function setFontSize(size: string) {
    execCommand('fontSize', size);
    showFontSizePicker = false;
  }

  function setTextColor(color: string) {
    execCommand('foreColor', color);
    showColorPicker = false;
  }

  function setBackgroundColor(color: string) {
    execCommand('backColor', color);
  }

  function insertList(type: 'ul' | 'ol') {
    execCommand(type === 'ul' ? 'insertUnorderedList' : 'insertOrderedList');
  }

  function insertLink() {
    const url = prompt('请输入链接地址:');
    if (url) {
      execCommand('createLink', url);
    }
  }

  function insertImage() {
    const url = prompt('请输入图片地址:');
    if (url) {
      execCommand('insertImage', url);
    }
  }

  // 获取编辑器内容
  function getEditorContent(): string {
    return editorElement?.innerHTML || '';
  }

  // 设置编辑器内容
  function setEditorContent(content: string) {
    if (editorElement) {
      editorElement.innerHTML = content;
    }
  }

  async function sendEmail() {
    if (!selectedSenderAccountId) {
      addNotification('error', '请选择发件人账户');
      return;
    }

    // 找到选中的发件人账户
    const senderAccount = $accounts.find(acc => acc.id === selectedSenderAccountId);
    if (!senderAccount) {
      addNotification('error', '选中的发件人账户不存在');
      return;
    }

    if (!to.trim()) {
      addNotification('error', '请输入收件人');
      return;
    }

    if (!subject.trim()) {
      addNotification('error', '请输入邮件主题');
      return;
    }

    // 获取富文本编辑器内容
    const htmlContent = getEditorContent();
    const textContent = editorElement?.textContent || '';

    if (!textContent.trim()) {
      addNotification('error', '请输入邮件内容');
      return;
    }

    isSending = true;

    try {
      // 解析收件人、抄送、密送
      const toList = to.split(',').map(email => email.trim()).filter(email => email);
      const ccList = cc ? cc.split(',').map(email => email.trim()).filter(email => email) : [];
      const bccList = bcc ? bcc.split(',').map(email => email.trim()).filter(email => email) : [];

      // 验证邮箱地址格式
      if (!validateEmailList(toList, '收件人')) {
        isSending = false;
        return;
      }
      if (ccList.length > 0 && !validateEmailList(ccList, '抄送')) {
        isSending = false;
        return;
      }
      if (bccList.length > 0 && !validateEmailList(bccList, '密送')) {
        isSending = false;
        return;
      }

      // 在转发模式下，合并用户输入和转发内容
      let finalHtmlContent = htmlContent;
      let finalTextContent = textContent.trim();

      if (isForwardMode && initialBody) {
        // 转发模式：用户输入 + 转发内容
        if (htmlContent.trim()) {
          // 用户有输入：用户输入 + 转发内容，减少间距
          finalHtmlContent = htmlContent + '<br>' + initialBody;
        } else {
          // 用户无输入：只有转发内容
          finalHtmlContent = initialBody;
        }

        // 纯文本版本：从HTML提取
        const tempDiv = document.createElement('div');
        tempDiv.innerHTML = finalHtmlContent;
        finalTextContent = tempDiv.textContent || tempDiv.innerText || '';
      }

      // 构建邮件发送请求
      const emailRequest = {
        account_id: selectedSenderAccountId,
        to: toList,
        cc: ccList,
        bcc: bccList,
        subject: subject.trim(),
        body_text: finalTextContent,
        body_html: finalHtmlContent,
        attachments: [],
        is_forward: isForwardMode, // 标识这是一个转发邮件
        forward_message_id: isForwardMode ? forwardEmail?.message_id : undefined // 转发的原始邮件ID
      };

      // 转发模式：不设置回复相关的头部信息，避免Outlook自动添加引用
      // 普通邮件和回复邮件可能需要设置in_reply_to和references
      if (!isForwardMode) {
        // 非转发模式下，可以设置回复头部（如果有的话）
        // emailRequest.in_reply_to = ...;
        // emailRequest.references = ...;
      }

      await EmailAPI.sendEmail(selectedSenderAccountId, emailRequest);

      // 如果是草稿模式，发送成功后删除原草稿
      if (isDraftMode && draftId) {
        try {
          await EmailAPI.deleteMessage(selectedSenderAccountId, draftId, 'DRAFTS');
        } catch (deleteError) {
          console.warn('删除原草稿失败:', deleteError);
          // 不影响主流程，只记录警告
        }
      }

      addNotification('success', isDraftMode ? '草稿发送成功' : '邮件发送成功');
      closeDialog();
    } catch (error) {
      console.error('发送邮件失败:', error);
      addNotification('error', `发送邮件失败: ${error}`);
    } finally {
      isSending = false;
    }
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      closeDialog();
    }
  }
</script>

<svelte:window on:keydown={handleKeydown} />

{#if show}
  <div class="dialog-overlay" role="dialog" aria-modal="true" tabindex="-1" on:click={closeDialog} on:keydown={handleKeydown}>
    <!-- svelte-ignore a11y-click-events-have-key-events -->
    <!-- svelte-ignore a11y-no-static-element-interactions -->
    <div class="dialog-content" on:click|stopPropagation>
      <div class="dialog-header">
        <h2>{isDraftMode ? '编辑草稿' : '写邮件'}</h2>
      </div>

      <div class="dialog-body">
        <!-- 左侧：收件人信息 -->
        <div class="left-section">
          <!-- 发件人选择 -->
          {#if $accounts && $accounts.length > 0}
            <div class="form-group">
              <label for="sender">发件人</label>
              <select id="sender" bind:value={selectedSenderAccountId}>
                {#each $accounts as account (account.id)}
                  <option value={account.id}>
                    {account.name} &lt;{account.email}&gt;
                  </option>
                {/each}
              </select>
            </div>
          {/if}

          <div class="form-group">
            <label for="to">收件人 *</label>
            <input
              id="to"
              type="email"
              bind:value={to}
              placeholder="请输入收件人邮箱，多个邮箱用逗号分隔"
              class:invalid={!toValid}
              required
            />
            {#if !toValid}
              <div class="error-message">请输入有效的邮箱地址格式，如：user@example.com</div>
            {/if}
          </div>

          <div class="form-group">
            <label for="cc">抄送</label>
            <input
              id="cc"
              type="email"
              bind:value={cc}
              placeholder="请输入抄送邮箱，多个邮箱用逗号分隔"
              class:invalid={!ccValid}
            />
            {#if !ccValid}
              <div class="error-message">请输入有效的邮箱地址格式，如：user@example.com</div>
            {/if}
          </div>

          <div class="form-group">
            <label for="bcc">密送</label>
            <input
              id="bcc"
              type="email"
              bind:value={bcc}
              placeholder="请输入密送邮箱，多个邮箱用逗号分隔"
              class:invalid={!bccValid}
            />
            {#if !bccValid}
              <div class="error-message">请输入有效的邮箱地址格式，如：user@example.com</div>
            {/if}
          </div>

          <div class="form-group">
            <label for="subject">主题 *</label>
            <input
              id="subject"
              type="text"
              bind:value={subject}
              placeholder="请输入邮件主题"
              required
            />
          </div>
        </div>

        <!-- 右侧：邮件内容 -->
        <div class="right-section">
          <div class="form-group content-group">
            <label for="body">内容 *</label>

            <!-- 富文本编辑器工具栏 -->
            <div class="editor-toolbar">
              <!-- 字体格式 -->
              <div class="toolbar-group">
                <button type="button" class="toolbar-btn" on:click={() => formatText('bold')} title="粗体">
                  <strong>B</strong>
                </button>
                <button type="button" class="toolbar-btn" on:click={() => formatText('italic')} title="斜体">
                  <em>I</em>
                </button>
                <button type="button" class="toolbar-btn" on:click={() => formatText('underline')} title="下划线">
                  <u>U</u>
                </button>
                <button type="button" class="toolbar-btn" on:click={() => formatText('strikeThrough')} title="删除线">
                  <s>S</s>
                </button>
              </div>

              <!-- 字体大小 -->
              <div class="toolbar-group">
                <div class="dropdown">
                  <button type="button" class="toolbar-btn dropdown-btn" on:click={() => showFontSizePicker = !showFontSizePicker} title="字体大小">
                    字号
                  </button>
                  {#if showFontSizePicker}
                    <div class="dropdown-menu">
                      <button type="button" on:click={() => setFontSize('1')}>小</button>
                      <button type="button" on:click={() => setFontSize('3')}>正常</button>
                      <button type="button" on:click={() => setFontSize('5')}>大</button>
                      <button type="button" on:click={() => setFontSize('7')}>特大</button>
                    </div>
                  {/if}
                </div>
              </div>

              <!-- 颜色 -->
              <div class="toolbar-group">
                <div class="dropdown">
                  <button type="button" class="toolbar-btn dropdown-btn" on:click={() => showColorPicker = !showColorPicker} title="文字颜色">
                    🎨
                  </button>
                  {#if showColorPicker}
                    <div class="dropdown-menu color-picker">
                      <!-- svelte-ignore a11y-click-events-have-key-events -->
                      <!-- svelte-ignore a11y-no-static-element-interactions -->
                      <button type="button" class="color-btn" style="background: #000000" on:click={() => setTextColor('#000000')} aria-label="黑色"></button>
                      <button type="button" class="color-btn" style="background: #ff0000" on:click={() => setTextColor('#ff0000')} aria-label="红色"></button>
                      <button type="button" class="color-btn" style="background: #00ff00" on:click={() => setTextColor('#00ff00')} aria-label="绿色"></button>
                      <button type="button" class="color-btn" style="background: #0000ff" on:click={() => setTextColor('#0000ff')} aria-label="蓝色"></button>
                      <button type="button" class="color-btn" style="background: #ffff00" on:click={() => setTextColor('#ffff00')} aria-label="黄色"></button>
                      <button type="button" class="color-btn" style="background: #ff00ff" on:click={() => setTextColor('#ff00ff')} aria-label="紫色"></button>
                      <button type="button" class="color-btn" style="background: #00ffff" on:click={() => setTextColor('#00ffff')} aria-label="青色"></button>
                      <button type="button" class="color-btn" style="background: #808080" on:click={() => setTextColor('#808080')} aria-label="灰色"></button>
                    </div>
                  {/if}
                </div>
              </div>

              <!-- 对齐 -->
              <div class="toolbar-group">
                <button type="button" class="toolbar-btn" on:click={() => formatText('justifyLeft')} title="左对齐">
                  ⬅️
                </button>
                <button type="button" class="toolbar-btn" on:click={() => formatText('justifyCenter')} title="居中">
                  ↔️
                </button>
                <button type="button" class="toolbar-btn" on:click={() => formatText('justifyRight')} title="右对齐">
                  ➡️
                </button>
              </div>

              <!-- 列表 -->
              <div class="toolbar-group">
                <button type="button" class="toolbar-btn" on:click={() => insertList('ul')} title="无序列表">
                  • 列表
                </button>
                <button type="button" class="toolbar-btn" on:click={() => insertList('ol')} title="有序列表">
                  1. 列表
                </button>
              </div>

              <!-- 链接和图片 -->
              <div class="toolbar-group">
                <button type="button" class="toolbar-btn" on:click={insertLink} title="插入链接">
                  🔗
                </button>
                <button type="button" class="toolbar-btn" on:click={insertImage} title="插入图片">
                  🖼️
                </button>
              </div>
            </div>

            <!-- 富文本编辑器 -->
            <div
              bind:this={editorElement}
              class="rich-editor separated"
              class:forward-mode={isForwardMode}
              contenteditable="true"
              data-placeholder={isForwardMode ? "请输入转发内容..." : "请输入邮件内容..."}
              on:input={() => {
                body = editorElement?.textContent || '';
                // 转发模式下检查第一个段落是否有内容
                if (isForwardMode && editorElement) {
                  const firstP = editorElement.querySelector('p');
                  const hasUserInput = firstP && firstP.textContent && firstP.textContent.trim().length > 0;
                  if (hasUserInput) {
                    editorElement.classList.add('has-user-input');
                  } else {
                    editorElement.classList.remove('has-user-input');
                  }
                }
              }}
            ></div>
          </div>

          <!-- 转发模式下的只读原始邮件内容 -->
          {#if isForwardMode && initialBody}
            <div class="forward-content-readonly">
              {@html initialBody}
            </div>
          {/if}
        </div>
      </div>

      <div class="dialog-footer">
        <button class="cancel-btn" on:click={closeDialog} disabled={isSending}>
          取消
        </button>
        <button class="send-btn" on:click={sendEmail} disabled={isSending}>
          {#if isSending}
            <svg class="spinner" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M21 12a9 9 0 11-6.219-8.56"/>
            </svg>
            发送中...
          {:else}
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <line x1="22" y1="2" x2="11" y2="13"></line>
              <polygon points="22,2 15,22 11,13 2,9 22,2"></polygon>
            </svg>
            发送
          {/if}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  /* CSS变量定义 - 这些会被主题切换器动态更新 */
  :global(:root) {
    --theme-primary: #6c7ae0;
    --theme-secondary: #7b68ee;
    --theme-hover: #5a68d4;
    --theme-hover-secondary: #6c5ce7;
  }

  .dialog-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
    backdrop-filter: blur(4px);
  }

  .dialog-content {
    background: linear-gradient(135deg, #ffffff 0%, #f8f9fa 100%);
    border-radius: 20px;
    box-shadow:
      0 25px 50px rgba(0, 0, 0, 0.15),
      0 10px 20px rgba(0, 0, 0, 0.1),
      0 0 0 1px rgba(255, 255, 255, 0.8);
    width: 95%;
    max-width: 1000px;
    max-height: 85vh;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    backdrop-filter: blur(10px);
    border: 1px solid rgba(255, 255, 255, 0.2);
  }

  .dialog-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px 20px;
    border-bottom: 1px solid rgba(0, 0, 0, 0.08);
    background: linear-gradient(135deg, #f8faff 0%, #f1f5ff 100%);
    color: #374151;
    position: relative;
  }

  .dialog-header h2 {
    margin: 0;
    color: var(--theme-primary);
    font-size: 16px;
    font-weight: 600;
  }

  .dialog-body {
    padding: 24px;
    overflow-y: auto;
    flex: 1;
    display: flex !important; /* 改用flex布局测试 */
    flex-direction: row !important;
    gap: 32px;
    min-height: 500px;
  }

  .left-section {
    display: flex;
    flex-direction: column;
    gap: 20px;
    flex: 1 !important; /* 占1份，50% */
    min-width: 0;
  }

  .right-section {
    display: flex;
    flex-direction: column;
    flex: 1 !important; /* 占1份，50% */
    min-width: 0;
  }

  .content-group {
    display: flex;
    flex-direction: column;
    flex: 1;
  }



  .form-group {
    margin-bottom: 0;
  }

  .form-group label {
    display: block;
    margin-bottom: 8px;
    font-weight: 600;
    color: #1f2937;
    font-size: 0.9rem;
    letter-spacing: 0.025em;
  }

  .form-group input {
    width: 100%;
    padding: 12px 16px;
    border: 2px solid #e5e7eb;
    border-radius: 12px;
    font-size: 0.9rem;
    transition: all 0.3s ease;
    box-sizing: border-box;
    background: #fafafa;
  }

  .form-group input:focus {
    outline: none;
    border-color: #667eea;
    box-shadow: 0 0 0 4px rgba(102, 126, 234, 0.1);
    background: white;
    transform: translateY(-1px);
  }

  .form-group input:hover {
    border-color: #d1d5db;
    background: white;
  }

  /* 错误状态样式 */
  .form-group input.invalid {
    border-color: #ef4444;
    background: #fef2f2;
    box-shadow: 0 0 0 3px rgba(239, 68, 68, 0.1);
  }

  .form-group input.invalid:focus {
    border-color: #ef4444;
    box-shadow: 0 0 0 4px rgba(239, 68, 68, 0.2);
  }

  .error-message {
    margin-top: 6px;
    font-size: 0.8rem;
    color: #ef4444;
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .error-message::before {
    content: '⚠️';
    font-size: 0.9rem;
  }

  /* 下拉选择框样式 */
  .form-group select {
    width: 100%;
    padding: 12px 40px 12px 16px; /* 右侧增加更多内边距给箭头留空间 */
    border: 2px solid #e5e7eb;
    border-radius: 12px;
    font-size: 0.9rem;
    transition: all 0.3s ease;
    box-sizing: border-box;
    background: #fafafa;
    cursor: pointer;
    -webkit-appearance: none; /* Webkit浏览器 */
    -moz-appearance: none; /* Firefox */
    appearance: none; /* 标准属性 */
    background-image: url("data:image/svg+xml;charset=UTF-8,%3csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='%236b7280' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3e%3cpolyline points='6,9 12,15 18,9'%3e%3c/polyline%3e%3c/svg%3e");
    background-repeat: no-repeat;
    background-position: right 12px center; /* 箭头距离右边框12px */
    background-size: 16px;
  }

  /* 针对IE浏览器隐藏默认箭头 */
  .form-group select::-ms-expand {
    display: none;
  }

  .form-group select:focus {
    outline: none;
    border-color: #667eea;
    box-shadow: 0 0 0 4px rgba(102, 126, 234, 0.1);
    background-color: white;
    background-image: url("data:image/svg+xml;charset=UTF-8,%3csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='%23667eea' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3e%3cpolyline points='6,9 12,15 18,9'%3e%3c/polyline%3e%3c/svg%3e");
    transform: translateY(-1px);
  }

  .form-group select:hover {
    border-color: #d1d5db;
    background-color: white;
    background-image: url("data:image/svg+xml;charset=UTF-8,%3csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='%23374151' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3e%3cpolyline points='6,9 12,15 18,9'%3e%3c/polyline%3e%3c/svg%3e");
  }



  /* 富文本编辑器样式 */
  .editor-toolbar {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    padding: 12px;
    background: linear-gradient(135deg, #f8f9fa, #e9ecef);
    border: 2px solid #e5e7eb;
    border-radius: 12px;
    margin-bottom: 12px;
    box-shadow: 0 2px 4px rgba(0,0,0,0.05);
  }

  .toolbar-group {
    display: flex;
    gap: 4px;
    position: relative;
  }

  .toolbar-btn {
    padding: 6px 10px;
    border: 1px solid #d1d5db;
    background: white;
    border-radius: 6px;
    cursor: pointer;
    font-size: 0.85rem;
    transition: all 0.2s ease;
    min-width: 32px;
    height: 32px;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .toolbar-btn:hover {
    background: #f3f4f6;
    border-color: #9ca3af;
  }

  .toolbar-btn:active {
    background: #e5e7eb;
  }

  .dropdown {
    position: relative;
  }

  .dropdown-menu {
    position: absolute;
    top: 100%;
    left: 0;
    background: white;
    border: 1px solid #d1d5db;
    border-radius: 8px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
    z-index: 1000;
    min-width: 120px;
    padding: 4px;
  }

  .dropdown-menu button {
    width: 100%;
    padding: 8px 12px;
    border: none;
    background: none;
    text-align: left;
    cursor: pointer;
    border-radius: 4px;
    font-size: 0.85rem;
  }

  .dropdown-menu button:hover {
    background: #f3f4f6;
  }

  .color-picker {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 4px;
    padding: 8px;
  }

  .color-btn {
    width: 24px;
    height: 24px;
    border: 1px solid #d1d5db;
    border-radius: 4px;
    cursor: pointer;
    transition: transform 0.2s ease;
  }

  .color-btn:hover {
    transform: scale(1.1);
    border-color: #6b7280;
  }

  .rich-editor {
    padding: 16px;
    border: 2px solid #e5e7eb;
    border-radius: 12px;
    background: white;
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', 'Roboto', 'Oxygen', 'Ubuntu', 'Cantarell', 'Fira Sans', 'Droid Sans', 'Helvetica Neue', sans-serif;
    font-size: 0.9rem;
    line-height: 1.6;
    outline: none;
    transition: all 0.3s ease;
    min-height: 300px; /* 为普通写邮件模式设置合适的最小高度 */
    overflow-y: auto; /* 内容过多时显示滚动条 */
  }

  .rich-editor.forward-mode {
    min-height: 200px;
    height: 200px;
    overflow-y: auto;
    resize: none;
  }

  .rich-editor:focus {
    border-color: #667eea;
    box-shadow: 0 0 0 4px rgba(102, 126, 234, 0.1);
    transform: translateY(-1px);
  }

  .rich-editor:hover {
    border-color: #d1d5db;
  }

  /* 占位符效果 - 统一处理 */
  .rich-editor[contenteditable]:empty::before {
    content: attr(data-placeholder);
    color: #9ca3af;
    font-style: italic;
    pointer-events: none;
    position: absolute;
    top: 12px;
    left: 12px;
  }



  /* 转发内容只读显示区域 */
  .forward-content-readonly {
    margin-top: 20px;
    padding: 16px;
    background: #f9fafb;
    border: 1px solid #e5e7eb;
    border-radius: 8px;
    font-size: 0.9rem;
    line-height: 1.6;
    color: #6b7280;
    max-height: 300px;
    overflow-y: auto;
    user-select: text;
    cursor: default;
    flex-shrink: 0;
  }

  .forward-content-readonly :global(p) {
    margin: 8px 0;
  }

  .forward-content-readonly :global(strong),
  .forward-content-readonly :global(b) {
    color: #374151;
    font-weight: 600;
  }

  /* 支持转发格式中的分隔线 */
  .forward-content-readonly :global(hr) {
    border: none;
    border-top: solid #E1E1E1 1.0pt;
    margin: 8px 0;
  }

  /* 支持转发格式中的div样式 */
  .forward-content-readonly :global(div) {
    margin: 4px 0;
  }

  /* 保持转发格式中的字体设置 */
  .forward-content-readonly :global([style*="font-family"]) {
    font-family: inherit !important;
  }



  .dialog-footer {
    display: flex;
    justify-content: flex-end;
    gap: 12px;
    padding: 12px 20px;
    border-top: 1px solid rgba(0, 0, 0, 0.06);
    background: linear-gradient(135deg, #f8fafc 0%, #f1f5f9 100%);
    flex-shrink: 0;
    position: relative;
  }

  .dialog-footer::before {
    content: '';
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 1px;
    background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.8), transparent);
  }

  .cancel-btn,
  .send-btn {
    padding: 8px 20px;
    border: none;
    border-radius: 10px;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.3s ease;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    position: relative;
    overflow: hidden;
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .cancel-btn::before,
  .send-btn::before {
    content: '';
    position: absolute;
    top: 0;
    left: -100%;
    width: 100%;
    height: 100%;
    background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.3), transparent);
    transition: left 0.5s ease;
  }

  .cancel-btn:hover::before,
  .send-btn:hover::before {
    left: 100%;
  }

  .cancel-btn {
    background: linear-gradient(135deg, #6b7280 0%, #4b5563 100%);
    color: white;
    box-shadow: 0 4px 12px rgba(107, 114, 128, 0.3);
  }



  .send-btn {
    background: linear-gradient(135deg, var(--theme-primary) 0%, var(--theme-secondary) 100%);
    color: white;
    box-shadow: 0 4px 12px rgba(108, 122, 224, 0.3);
  }



  .send-btn:disabled,
  .cancel-btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .spinner {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from {
      transform: rotate(0deg);
    }
    to {
      transform: rotate(360deg);
    }
  }

  /* 响应式设计 */
  @media (max-width: 400px) {
    .dialog-content {
      max-width: 95%;
      margin: 20px;
    }

    .dialog-body {
      /* grid-template-columns: 1fr; 注释掉，不强制单列 */
      gap: 24px;
      min-height: auto;
    }

    .rich-editor {
      min-height: 200px;
    }

    .editor-toolbar {
      flex-wrap: wrap;
      gap: 4px;
      padding: 8px;
    }

    .toolbar-btn {
      min-width: 28px;
      height: 28px;
      font-size: 0.8rem;
    }

    .dialog-footer {
      padding: 20px 24px;
      flex-direction: column-reverse;
    }

    .cancel-btn,
    .send-btn {
      width: 100%;
      justify-content: center;
    }
  }

  /* 转发对话框自定义滚动条样式 */
  .dialog-body::-webkit-scrollbar,
  .rich-editor::-webkit-scrollbar {
    width: 8px;
  }

  .dialog-body::-webkit-scrollbar-track,
  .rich-editor::-webkit-scrollbar-track {
    background: #f1f5f9;
    border-radius: 4px;
  }

  .dialog-body::-webkit-scrollbar-thumb,
  .rich-editor::-webkit-scrollbar-thumb {
    background: linear-gradient(135deg, #cbd5e1 0%, #94a3b8 100%);
    border-radius: 4px;
    transition: background 0.3s ease;
  }

  .dialog-body::-webkit-scrollbar-thumb:hover,
  .rich-editor::-webkit-scrollbar-thumb:hover {
    background: linear-gradient(135deg, #94a3b8 0%, #64748b 100%);
  }

  /* 移除滚动条箭头按钮 */
  .dialog-body::-webkit-scrollbar-button,
  .rich-editor::-webkit-scrollbar-button {
    display: none;
  }

  /* Firefox 滚动条样式 */
  .dialog-body,
  .rich-editor {
    scrollbar-width: thin;
    scrollbar-color: #cbd5e1 #f1f5f9;
  }
</style>
