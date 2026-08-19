<script lang="ts">
  import { accounts, currentAccount, currentFolder, showAccountDialog, preselectedProvider } from '../store';
  import { addNotification } from '../store';
  import { EmailAPI } from '../api';
  import type { EmailProviderConfig } from '../types';
  import { onMount } from 'svelte';

  let folders = [
    { name: '收件箱', path: 'INBOX', icon: '📥', unread: 0 },
    { name: '已发送', path: 'Sent', icon: '📤', unread: 0 },
    { name: '草稿箱', path: 'DRAFTS', icon: '📝', unread: 0 },
    { name: '垃圾箱', path: 'Trash', icon: '🗑️', unread: 0 },
    { name: '已加星标', path: 'Starred', icon: '⭐', unread: 0 }
  ];

  let providers: EmailProviderConfig[] = [];

  // 加载邮箱提供商列表
  onMount(async () => {
    try {
      providers = await EmailAPI.getEmailProviders();
      console.log('加载的邮箱提供商:', providers);
    } catch (error) {
      console.error('加载邮箱提供商失败:', error);
    }
  });

  async function selectAccount(account: any) {
    currentAccount.set(account);
    currentFolder.set('INBOX');
    // 不在这里加载邮件，让InnovativeEmailInterface组件响应账户变化来处理
    addNotification('success', `已切换到账户: ${account.name}`);
  }

  function selectFolder(folder: any) {
    currentFolder.set(folder.path);
    addNotification('info', `已切换到文件夹: ${folder.name}`);
  }

  function openAccountDialog() {
    showAccountDialog.set(true);
  }

  // 快速添加指定提供商的账户
  function quickAddProvider(provider: EmailProviderConfig) {
    // 设置预选的提供商，然后打开对话框
    preselectedProvider.set(provider);
    showAccountDialog.set(true);
    addNotification('info', `准备添加 ${provider.name} 账户`);
  }

  // 获取提供商图标
  function getProviderIcon(providerName: string): string {
    const icons: Record<string, string> = {
      'Gmail': '📧',
      'Outlook': '📮',
      'QQ邮箱': '🐧',
      '163邮箱': '📬',
      '126邮箱': '📭',
      'Yahoo邮箱': '📫',
      'iCloud': '☁️'
    };
    return icons[providerName] || '📧';
  }
</script>

<div class="sidebar">
  <div class="sidebar-header">
    <h2>📧 XMail</h2>
    <button class="add-account-btn" on:click={openAccountDialog}>
      ➕ 添加账户
    </button>
  </div>

  <div class="accounts-section">
    <h3>邮箱账户</h3>
    {#each $accounts as account}
      <button
        class="account-item"
        class:active={$currentAccount?.id === account.id}
        on:click={() => selectAccount(account)}
        aria-label="选择账户 {account.name}"
      >
        <div class="account-info">
          <div class="account-name">{account.name}</div>
          <div class="account-email">{account.email}</div>
        </div>
      </button>
    {/each}
    
    {#if $accounts.length === 0}
      <div class="no-accounts">
        <p class="no-accounts-title">选择邮箱提供商快速添加</p>
        <div class="debug-info">
          <p>调试信息：账户数量 = {$accounts.length}, 提供商数量 = {providers.length}</p>
        </div>
        <div class="provider-list">
          {#each providers as provider}
            <button
              class="provider-item"
              on:click={() => quickAddProvider(provider)}
              aria-label="添加 {provider.name} 账户"
            >
              <span class="provider-icon">{getProviderIcon(provider.name)}</span>
              <span class="provider-name">{provider.name}</span>
              <span class="add-icon">+</span>
            </button>
          {/each}
        </div>
        <div class="manual-add">
          <button class="manual-add-btn" on:click={openAccountDialog}>
            ⚙️ 手动配置其他邮箱
          </button>
        </div>
      </div>
    {:else}
      <div class="debug-info">
        <p>调试信息：有 {$accounts.length} 个账户</p>
      </div>
    {/if}
  </div>

  {#if $currentAccount}
    <div class="folders-section">
      <h3>文件夹</h3>
      {#each folders as folder}
        <button
          class="folder-item"
          class:active={$currentFolder === folder.path}
          on:click={() => selectFolder(folder)}
          aria-label="选择文件夹 {folder.name}"
        >
          <span class="folder-icon">{folder.icon}</span>
          <span class="folder-name">{folder.name}</span>
          {#if folder.unread > 0}
            <span class="unread-count">{folder.unread}</span>
          {/if}
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .sidebar {
    width: 280px;
    height: 100vh;
    background: #f8f9fa;
    border-right: 1px solid #e9ecef;
    display: flex;
    flex-direction: column;
    overflow-y: auto;
  }

  .sidebar-header {
    padding: 20px;
    border-bottom: 1px solid #e9ecef;
  }

  .sidebar-header h2 {
    margin: 0 0 15px 0;
    color: #495057;
    font-size: 1.5rem;
  }

  .add-account-btn {
    width: 100%;
    padding: 10px;
    background: #007bff;
    color: white;
    border: none;
    border-radius: 6px;
    cursor: pointer;
    font-size: 14px;
    transition: background-color 0.2s;
  }

  .add-account-btn:hover {
    background: #0056b3;
  }

  .accounts-section, .folders-section {
    padding: 20px;
  }

  .accounts-section h3, .folders-section h3 {
    margin: 0 0 15px 0;
    color: #6c757d;
    font-size: 0.9rem;
    text-transform: uppercase;
    font-weight: 600;
  }

  .account-item {
    width: 100%;
    padding: 12px;
    border: none;
    border-radius: 6px;
    background: transparent;
    cursor: pointer;
    margin-bottom: 8px;
    transition: background-color 0.2s;
    text-align: left;
  }

  .account-item:hover {
    background: #e9ecef;
  }

  .account-item.active {
    background: #007bff;
    color: white;
  }

  .account-info {
    display: flex;
    flex-direction: column;
  }

  .account-name {
    font-weight: 600;
    font-size: 14px;
  }

  .account-email {
    font-size: 12px;
    opacity: 0.8;
    margin-top: 2px;
  }

  .folder-item {
    width: 100%;
    display: flex;
    align-items: center;
    padding: 10px 12px;
    border: none;
    border-radius: 6px;
    background: transparent;
    cursor: pointer;
    margin-bottom: 4px;
    transition: background-color 0.2s;
    text-align: left;
  }

  .folder-item:hover {
    background: #e9ecef;
  }

  .folder-item.active {
    background: #007bff;
    color: white;
  }

  .folder-icon {
    margin-right: 10px;
    font-size: 16px;
  }

  .folder-name {
    flex: 1;
    font-size: 14px;
  }

  .unread-count {
    background: #dc3545;
    color: white;
    border-radius: 10px;
    padding: 2px 6px;
    font-size: 11px;
    font-weight: 600;
    min-width: 18px;
    text-align: center;
  }

  .no-accounts {
    color: #6c757d;
    font-size: 14px;
    padding: 10px 0;
  }

  .no-accounts-title {
    text-align: center;
    margin: 0 0 15px 0;
    font-weight: 600;
    color: #495057;
  }

  .provider-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-bottom: 15px;
  }

  .provider-item {
    width: 100%;
    display: flex;
    align-items: center;
    padding: 10px 12px;
    border: 1px solid #e9ecef;
    border-radius: 6px;
    background: white;
    cursor: pointer;
    transition: all 0.2s;
    text-align: left;
  }

  .provider-item:hover {
    background: #f8f9fa;
    border-color: #007bff;
    transform: translateY(-1px);
    box-shadow: 0 2px 4px rgba(0,123,255,0.1);
  }

  .provider-icon {
    margin-right: 10px;
    font-size: 16px;
  }

  .provider-name {
    flex: 1;
    font-size: 14px;
    color: #495057;
    font-weight: 500;
  }

  .add-icon {
    color: #007bff;
    font-size: 18px;
    font-weight: bold;
  }

  .manual-add {
    text-align: center;
    padding-top: 10px;
    border-top: 1px solid #e9ecef;
  }

  .manual-add-btn {
    padding: 8px 16px;
    background: transparent;
    color: #6c757d;
    border: 1px solid #e9ecef;
    border-radius: 6px;
    cursor: pointer;
    font-size: 12px;
    transition: all 0.2s;
  }

  .manual-add-btn:hover {
    background: #f8f9fa;
    color: #495057;
    border-color: #007bff;
  }

  .debug-info {
    background: #fff3cd;
    border: 1px solid #ffeaa7;
    border-radius: 4px;
    padding: 8px;
    margin: 10px 0;
    font-size: 12px;
    color: #856404;
  }
</style>
