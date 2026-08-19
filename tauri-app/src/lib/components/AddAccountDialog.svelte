<script lang="ts">
  import { showAccountDialog, accounts, currentAccount, currentFolder, preselectedProvider } from '../store';
  import { EmailAPI } from '../api';
  import { addNotification } from '../store';
  import type { NewEmailAccount, EmailProviderConfig } from '../types';
  import { invoke } from '@tauri-apps/api/core';

  let formData: NewEmailAccount = {
    name: '',
    email: '',
    imap_server: '',
    imap_port: 993,
    smtp_server: '',
    smtp_port: 587,
    username: '',
    password: '',
    use_tls: true
  };

  let providers: EmailProviderConfig[] = [];
  let selectedProvider: EmailProviderConfig | null = null;
  let isLoading = false;
  let step = 1; // 1: 选择提供商, 2: 填写详细信息, 3: 测试连接, 4: 测试成功, 5: 测试失败
  let testError = '';
  let emailValidationError = '';

  // OAuth2 相关变量
  let isOAuth2Flow = false;
  let oauthToken: any = null;

  // 判断是否为Gmail或Outlook
  $: isGmail = selectedProvider?.name === 'Gmail';
  $: isOutlook = selectedProvider?.name === 'Outlook';

  // 将技术错误信息转换为用户友好的提示
  function getUserFriendlyError(error: string): string {
    const errorLower = error.toLowerCase();

    // QQ邮箱相关错误
    if (errorLower.includes('qq.com') || errorLower.includes('service.mail.qq.com')) {
      if (errorLower.includes('password is incorrect') || errorLower.includes('login fail')) {
        return '密码错误或服务未开启。QQ邮箱需要使用授权码，不是QQ密码。请到QQ邮箱设置中开启IMAP/SMTP服务并生成授权码。';
      }
    }

    // 163邮箱相关错误
    if (errorLower.includes('163.com')) {
      if (errorLower.includes('password') || errorLower.includes('login')) {
        return '密码错误或服务未开启。网易邮箱需要使用客户端授权密码，不是邮箱登录密码。请到邮箱设置中开启IMAP/SMTP服务并设置客户端授权密码。';
      }
    }

    // Gmail相关错误
    if (errorLower.includes('gmail.com')) {
      if (errorLower.includes('password') || errorLower.includes('authentication')) {
        return '身份验证失败。从2025年开始，Gmail不再支持应用密码，必须使用OAuth2授权。请联系开发者获取OAuth2配置支持。';
      }
    }

    // 通用错误类型
    if (errorLower.includes('password is incorrect') || errorLower.includes('login fail') || errorLower.includes('authentication failed')) {
      return '用户名或密码错误。请检查邮箱地址和密码是否正确，如果是QQ、163等邮箱，请使用授权码而非登录密码。';
    }

    if (errorLower.includes('connection refused') || errorLower.includes('connection timeout')) {
      return '无法连接到邮箱服务器。请检查网络连接和服务器地址、端口号是否正确。';
    }

    if (errorLower.includes('ssl') || errorLower.includes('tls')) {
      return 'SSL/TLS连接失败。请检查是否启用了TLS加密，或尝试切换加密设置。';
    }

    if (errorLower.includes('service is not open') || errorLower.includes('service not available')) {
      return '邮箱服务未开启。请到邮箱设置中开启IMAP/SMTP服务。';
    }

    // 如果没有匹配的模式，返回简化的错误信息
    return '连接失败。请检查邮箱设置，确保用户名、密码正确，并已开启IMAP/SMTP服务。';
  }

  // 根据邮箱提供商返回合适的密码提示文字
  function getPasswordPlaceholder(): string {
    if (!selectedProvider) return '邮箱密码或应用专用密码';

    switch (selectedProvider.name) {
      case 'QQ邮箱':
        return 'QQ授权码（非QQ密码）';
      case '163邮箱':
      case '126邮箱':
        return '客户端授权密码（非邮箱登录密码）';
      case 'Gmail':
        return '应用专用密码（非Google账户密码）';
      case 'Outlook':
        return '应用密码或邮箱密码';
      default:
        return '邮箱密码或应用专用密码';
    }
  }

  // 根据邮箱提供商返回邮箱地址占位符
  function getEmailPlaceholder(): string {
    if (!selectedProvider) return 'your@email.com';

    switch (selectedProvider.name) {
      case 'QQ邮箱':
        return 'your@qq.com';
      case '163邮箱':
        return 'your@163.com';
      case '126邮箱':
        return 'your@126.com';
      case 'Gmail':
        return 'your@gmail.com';
      case 'Outlook':
        return 'your@outlook.com';
      case 'Yahoo邮箱':
        return 'your@yahoo.com';
      case 'iCloud':
        return 'your@icloud.com';
      default:
        return 'your@email.com';
    }
  }

  // 根据邮箱提供商返回IMAP服务器占位符
  function getImapPlaceholder(): string {
    if (!selectedProvider) return 'imap.example.com';
    return selectedProvider.imap_server;
  }

  // 根据邮箱提供商返回SMTP服务器占位符
  function getSmtpPlaceholder(): string {
    if (!selectedProvider) return 'smtp.example.com';
    return selectedProvider.smtp_server;
  }

  async function loadProviders() {
    try {
      providers = await EmailAPI.getEmailProviders();
    } catch (error) {
      addNotification('error', `加载邮箱提供商失败: ${error}`);
    }
  }

  function selectProvider(provider: EmailProviderConfig) {
    selectedProvider = provider;
    formData.imap_server = provider.imap_server;
    formData.imap_port = provider.imap_port;
    formData.smtp_server = provider.smtp_server;
    formData.smtp_port = provider.smtp_port;
    formData.use_tls = provider.use_tls;

    // 如果是Gmail或Outlook，直接跳转到OAuth2授权
    if (provider.name === 'Gmail') {
      step = 6; // Gmail专用步骤
    } else if (provider.name === 'Outlook') {
      step = 7; // Outlook专用步骤
    } else {
      step = 2;
    }
  }

  function useCustomProvider() {
    selectedProvider = null;
    step = 2;
  }



  // 验证邮箱地址是否与选择的提供商匹配
  function validateEmailProvider(): boolean {
    if (!selectedProvider || !formData.email) return true;

    const emailDomain = formData.email.split('@')[1]?.toLowerCase();
    if (!emailDomain) return false;

    // 定义提供商域名映射
    const providerDomains: Record<string, string[]> = {
      'Gmail': ['gmail.com'],
      'QQ邮箱': ['qq.com'],
      'Outlook': ['outlook.com'],
      '163邮箱': ['163.com'],
      '126邮箱': ['126.com'],
      'Yahoo邮箱': ['yahoo.com'],
      'iCloud': ['icloud.com']
    };

    const allowedDomains = providerDomains[selectedProvider.name];
    if (!allowedDomains) return true; // 如果没有定义域名限制，则允许

    const isValidDomain = allowedDomains.some(domain => emailDomain === domain);

    if (!isValidDomain) {
      addNotification('error', `${selectedProvider.name} 只支持 ${allowedDomains[0]} 域名`);
      return false;
    }

    return true;
  }

  // 实时验证邮箱地址
  function validateEmailRealtime() {
    emailValidationError = '';

    if (!selectedProvider || !formData.email) return;

    const emailDomain = formData.email.split('@')[1]?.toLowerCase();
    if (!emailDomain) return;

    // 定义提供商域名映射
    const providerDomains: Record<string, string[]> = {
      'Gmail': ['gmail.com'],
      'QQ邮箱': ['qq.com'],
      'Outlook': ['outlook.com'],
      '163邮箱': ['163.com'],
      '126邮箱': ['126.com'],
      'Yahoo邮箱': ['yahoo.com'],
      'iCloud': ['icloud.com']
    };

    const allowedDomains = providerDomains[selectedProvider.name];
    if (!allowedDomains) return;

    const isValidDomain = allowedDomains.some(domain => emailDomain === domain);

    if (!isValidDomain) {
      emailValidationError = `${selectedProvider.name} 只支持 ${allowedDomains[0]} 域名`;
    }
  }

  // 监听邮箱地址变化
  $: if (formData.email) {
    validateEmailRealtime();
  }

  async function testConnection() {
    if (!validateForm()) return;

    // 验证邮箱地址与提供商是否匹配
    if (!validateEmailProvider()) return;

    isLoading = true;
    step = 3;
    testError = '';

    try {
      // 直接测试连接，不创建账户
      const connectionOk = await EmailAPI.testEmailConnection(formData);

      if (connectionOk) {
        // 测试成功，进入成功页面
        step = 4;
      } else {
        // 测试失败，进入失败页面
        testError = '连接测试失败，请检查账户设置';
        step = 5;
      }
    } catch (error) {
      // 测试出错，进入失败页面
      testError = String(error);
      step = 5;
    } finally {
      isLoading = false;
    }
  }

  async function saveAccount() {
    try {
      isLoading = true;

      // 为Gmail或Outlook账户准备OAuth2数据
      let accountData = { ...formData };
      if (isGmail && oauthToken) {
        accountData = {
          ...formData,
          auth_type: 'oauth2',
          access_token: oauthToken.access_token,
          refresh_token: oauthToken.refresh_token,
          token_expires_at: oauthToken.expires_at,
          // Gmail服务器设置
          imap_server: 'imap.gmail.com',
          imap_port: 993,
          smtp_server: 'smtp.gmail.com',
          smtp_port: 465,
          use_tls: true
        };
      } else if (isOutlook && oauthToken) {
        accountData = {
          ...formData,
          auth_type: 'oauth2',
          access_token: oauthToken.access_token,
          refresh_token: oauthToken.refresh_token,
          token_expires_at: oauthToken.expires_at,
          // Outlook服务器设置
          imap_server: 'imap-mail.outlook.com',
          imap_port: 993,
          smtp_server: 'smtp-mail.outlook.com',
          smtp_port: 587,
          use_tls: true
        };
      }

      // 创建账户并保存到数据库
      const account = await EmailAPI.createAccount(accountData);
      accounts.update(accs => [...accs, account]);
      addNotification('success', `账户 ${formData.name} 添加成功！`);

      // 立即关闭对话框并切换到新账户
      closeDialog();

      // 自动选择新添加的账户
      currentAccount.set(account);
      currentFolder.set('INBOX');
      addNotification('info', `已切换到账户: ${account.name}`);

      // 在后台获取邮件（不阻塞界面）
      setTimeout(async () => {
        try {
          addNotification('info', '正在获取邮件...');
          await EmailAPI.getMessages(account.id, 'INBOX');
          addNotification('success', '邮件获取完成！');
        } catch (emailError) {
          addNotification('warning', `邮件获取失败: ${emailError}`);
        }
      }, 100); // 延迟100ms确保界面已经切换

    } catch (error) {
      // 处理重复邮箱地址错误
      const errorMessage = String(error);
      if (errorMessage.includes('Duplicate entry') && errorMessage.includes('email')) {
        addNotification('error', '该邮箱地址已存在，请使用其他邮箱地址');
      } else {
        addNotification('error', `保存账户失败: ${errorMessage}`);
      }
    } finally {
      isLoading = false;
    }
  }

  function validateForm(): boolean {
    if (!formData.name.trim()) {
      addNotification('error', '请输入账户名称');
      return false;
    }
    if (!formData.email.trim()) {
      addNotification('error', '请输入邮箱地址');
      return false;
    }

    // Gmail特殊验证
    if (isGmail) {
      if (!oauthToken) {
        addNotification('error', '请先完成Gmail OAuth2授权');
        return false;
      }
      return true; // Gmail只需要名称、邮箱和OAuth2令牌
    }

    // Outlook特殊验证
    if (isOutlook) {
      if (!oauthToken) {
        addNotification('error', '请先完成Outlook OAuth2授权');
        return false;
      }
      return true; // Outlook只需要名称、邮箱和OAuth2令牌
    }

    // 其他邮箱的完整验证
    if (!formData.username.trim()) {
      addNotification('error', '请输入用户名');
      return false;
    }
    if (!formData.password.trim()) {
      addNotification('error', '请输入密码');
      return false;
    }
    if (!formData.imap_server.trim()) {
      addNotification('error', '请输入IMAP服务器');
      return false;
    }
    if (!formData.smtp_server.trim()) {
      addNotification('error', '请输入SMTP服务器');
      return false;
    }
    return true;
  }

  function closeDialog() {
    showAccountDialog.set(false);
    resetForm();
  }

  function resetForm() {
    formData = {
      name: '',
      email: '',
      imap_server: '',
      imap_port: 993,
      smtp_server: '',
      smtp_port: 587,
      username: '',
      password: '',
      use_tls: true
    };
    selectedProvider = null;
    step = 1;
    isLoading = false;
  }

  // OAuth2 授权函数 - 使用弹窗方式
  async function startGmailOAuth() {
    // 先验证必填字段
    if (!formData.name.trim()) {
      addNotification('error', '请输入账户名称');
      return;
    }
    if (!formData.email.trim()) {
      addNotification('error', '请输入Gmail邮箱地址');
      return;
    }
    if (!formData.email.includes('@gmail.com')) {
      addNotification('error', '请输入有效的Gmail邮箱地址');
      return;
    }

    try {
      isLoading = true;
      addNotification('info', '正在启动Gmail OAuth2授权...');
      addNotification('info', '请在弹出的授权窗口中完成授权');

      // 打开OAuth2授权窗口并等待完成
      oauthToken = await invoke('start_oauth2_window') as any;

      // 设置表单数据为OAuth2模式
      formData.password = 'oauth2_token'; // 占位符
      formData.username = formData.email;

      addNotification('success', 'Gmail OAuth2授权成功！');

      // 自动保存账户（提供一致的用户体验）
      setTimeout(async () => {
        try {
          await saveAccount();
        } catch (error) {
          console.error('自动保存账户失败:', error);
        }
      }, 1000); // 1秒后自动保存

    } catch (error) {
      addNotification('error', `OAuth2授权失败: ${error}`);
    } finally {
      isLoading = false;
    }
  }

  // Outlook OAuth2 授权函数
  async function startOutlookOAuth() {
    // 先验证必填字段
    if (!formData.name.trim()) {
      addNotification('error', '请输入账户名称');
      return;
    }
    if (!formData.email.trim()) {
      addNotification('error', '请输入Outlook邮箱地址');
      return;
    }
    if (!formData.email.includes('@outlook.com') && !formData.email.includes('@hotmail.com') && !formData.email.includes('@live.com')) {
      addNotification('error', '请输入有效的Outlook邮箱地址');
      return;
    }

    try {
      isLoading = true;
      addNotification('info', '正在启动Outlook OAuth2授权...');
      addNotification('info', '请在弹出的授权窗口中完成授权');

      // 打开OAuth2授权窗口并等待完成
      oauthToken = await invoke('start_outlook_oauth2_window') as any;

      // 设置表单数据为OAuth2模式
      formData.password = 'oauth2_token'; // 占位符
      formData.username = formData.email;

      addNotification('success', 'Outlook OAuth2授权成功！');

      // 自动保存账户（与Gmail保持一致的体验）
      setTimeout(async () => {
        try {
          await saveAccount();
        } catch (error) {
          console.error('自动保存账户失败:', error);
        }
      }, 1000); // 1秒后自动保存

    } catch (error) {
      addNotification('error', `OAuth2授权失败: ${error}`);
    } finally {
      isLoading = false;
    }
  }

  // 取消OAuth2授权
  function cancelOAuth() {
    isLoading = false;
    addNotification('info', '已取消OAuth2授权');
  }



  // 当对话框打开时加载提供商列表
  $: if ($showAccountDialog) {
    loadProviders();

    // 检查是否有预选的提供商
    if ($preselectedProvider) {
      selectProvider($preselectedProvider);
      // 清除预选状态
      preselectedProvider.set(null);
    }
  }
</script>

{#if $showAccountDialog}
  <div
    class="dialog-overlay"
    on:click={(e) => {
      if (e.target === e.currentTarget) {
        closeDialog();
      }
    }}
    on:keydown={(e) => e.key === 'Escape' && closeDialog()}
    role="dialog"
    aria-modal="true"
    aria-labelledby="dialog-title"
    tabindex="-1"
  >
    <div
      class="dialog {step === 6 ? 'gmail-dialog' : ''} {step === 7 ? 'outlook-dialog' : ''}"
      role="document"
    >
      {#if step !== 6 && step !== 7}
        <div class="dialog-header">
          {#if step === 1}
            <h2 id="dialog-title">选择邮箱提供商</h2>
          {:else if step === 2}
            <div class="header-with-icon">
              {#if selectedProvider && selectedProvider.name === 'Outlook'}
                <img src="/Outlook.png" alt="Outlook" class="provider-header-icon" />
              {:else if selectedProvider && selectedProvider.name === 'Gmail'}
                <img src="/guge.png" alt="Gmail" class="provider-header-icon" />
              {:else if selectedProvider && selectedProvider.name === 'QQ邮箱'}
                <img src="/QQ.png" alt="QQ邮箱" class="provider-header-icon" />
              {:else if selectedProvider && selectedProvider.name === '163邮箱'}
                <img src="/163.png" alt="163邮箱" class="provider-header-icon" />
              {:else if selectedProvider && selectedProvider.name === '126邮箱'}
                <img src="/126.png" alt="126邮箱" class="provider-header-icon" />
              {:else if selectedProvider && selectedProvider.name === 'Yahoo邮箱'}
                <img src="/Yahoo.png" alt="Yahoo邮箱" class="provider-header-icon" />
              {:else if selectedProvider && selectedProvider.name === 'iCloud'}
                <img src="/cloud.png" alt="iCloud" class="provider-header-icon" />
              {:else}
                <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="default-email-icon">
                  <path d="M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z"></path>
                  <polyline points="22,6 12,13 2,6"></polyline>
                </svg>
              {/if}
              <h2>配置 {selectedProvider ? selectedProvider.name : '自定义邮箱'}</h2>
            </div>
          {:else if step === 3}
            <div class="header-with-icon">
              {#if selectedProvider && selectedProvider.name === 'Outlook'}
                <img src="/Outlook.png" alt="Outlook" class="provider-header-icon" />
              {:else if selectedProvider && selectedProvider.name === 'Gmail'}
                <img src="/guge.png" alt="Gmail" class="provider-header-icon" />
              {:else if selectedProvider && selectedProvider.name === 'QQ邮箱'}
                <img src="/QQ.png" alt="QQ邮箱" class="provider-header-icon" />
              {:else if selectedProvider && selectedProvider.name === '163邮箱'}
                <img src="/163.png" alt="163邮箱" class="provider-header-icon" />
              {:else if selectedProvider && selectedProvider.name === '126邮箱'}
                <img src="/126.png" alt="126邮箱" class="provider-header-icon" />
              {:else if selectedProvider && selectedProvider.name === 'Yahoo邮箱'}
                <img src="/Yahoo.png" alt="Yahoo邮箱" class="provider-header-icon" />
              {:else if selectedProvider && selectedProvider.name === 'iCloud'}
                <img src="/cloud.png" alt="iCloud" class="provider-header-icon" />
              {:else}
                <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="default-email-icon">
                  <path d="M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z"></path>
                  <polyline points="22,6 12,13 2,6"></polyline>
                </svg>
              {/if}
              <h2>测试连接</h2>
            </div>
          {:else if step === 4}
            <div class="header-with-icon">
              <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="success-icon">
                <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"></path>
                <polyline points="22,4 12,14.01 9,11.01"></polyline>
              </svg>
              <h2 id="dialog-title">测试成功</h2>
            </div>
          {:else if step === 5}
            <div class="header-with-icon">
              <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="error-icon">
                <circle cx="12" cy="12" r="10"></circle>
                <line x1="15" y1="9" x2="9" y2="15"></line>
                <line x1="9" y1="9" x2="15" y2="15"></line>
              </svg>
              <h2>测试失败</h2>
            </div>
          {/if}
          <button class="close-btn" on:click={closeDialog}>✕</button>
        </div>
      {:else}
        <!-- Gmail/Outlook步骤：只显示关闭按钮 -->
        <div class="dialog-header {step === 6 ? 'gmail-header-minimal' : 'outlook-header-minimal'}">
          <button class="close-btn" on:click={closeDialog}>✕</button>
        </div>
      {/if}

      <div class="dialog-content">
        {#if step === 1}
          <!-- 选择邮箱提供商 -->
          <div class="step-content">

            <div class="provider-grid">
              {#each providers as provider}
                <button
                  class="provider-card"
                  on:click={() => selectProvider(provider)}
                >
                  <div class="provider-icon">
                    {#if provider.name === 'Gmail'}
                      <img src="/guge.png" alt="{provider.name}" />
                    {:else if provider.name === 'QQ邮箱'}
                      <img src="/QQ.png" alt="{provider.name}" />
                    {:else if provider.name === 'Outlook'}
                      <img src="/Outlook.png" alt="{provider.name}" />
                    {:else if provider.name === '163邮箱'}
                      <img src="/163.png" alt="{provider.name}" />
                    {:else if provider.name === '126邮箱'}
                      <img src="/126.png" alt="{provider.name}" />
                    {:else if provider.name === 'Yahoo邮箱'}
                      <img src="/Yahoo.png" alt="{provider.name}" />
                    {:else if provider.name === 'iCloud'}
                      <img src="/cloud.png" alt="{provider.name}" />
                    {:else}
                      <div class="provider-fallback">{provider.name.charAt(0)}</div>
                    {/if}
                  </div>
                  <div class="provider-info">
                    <div class="provider-name">{provider.name}</div>
                    <div class="provider-details">
                      {#if provider.name === 'Gmail'}
                        Google 提供的免费邮箱服务，功能强大且安全可靠
                      {:else if provider.name === 'QQ邮箱'}
                        腾讯 QQ 邮箱，国内用户使用广泛
                      {:else if provider.name === 'Outlook'}
                        微软 Outlook 邮箱，企业级邮件服务
                      {:else if provider.name === '163邮箱'}
                        网易 163 邮箱，老牌国内邮箱服务商
                      {:else if provider.name === '126邮箱'}
                        网易 126 邮箱，稳定可靠的邮件服务
                      {:else if provider.name === 'Yahoo邮箱'}
                        Yahoo 邮箱，国际知名邮件服务
                      {:else if provider.name === 'iCloud'}
                        苹果 iCloud 邮箱，与 Apple 设备完美集成
                      {:else}
                        安全可靠的邮箱服务
                      {/if}
                    </div>
                    <div class="provider-config">
                      <span class="config-item">IMAP: {provider.imap_server}:{provider.imap_port}</span>
                      <span class="config-item">SMTP: {provider.smtp_server}:{provider.smtp_port}</span>
                    </div>
                  </div>
                  <div class="provider-arrow">→</div>
                </button>
              {/each}

              <button class="provider-card custom-card" on:click={useCustomProvider}>
                <div class="provider-icon custom-icon">
                  <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M12 20h9"></path>
                    <path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z"></path>
                  </svg>
                </div>
                <div class="provider-info">
                  <div class="provider-name">自定义设置</div>
                  <div class="provider-details">手动配置IMAP/SMTP服务器</div>
                  <div class="provider-config">
                    <span class="config-item">适用于企业邮箱或其他服务商</span>
                  </div>
                </div>
                <div class="provider-arrow">→</div>
              </button>
            </div>
          </div>

        {:else if step === 2}
          <!-- 填写账户信息 -->
          <div class="step-content">
            <form class="account-form" on:submit|preventDefault={testConnection}>
              <!-- Gmail 左右布局 -->
              {#if isGmail}
                <div class="form-content gmail-layout">
                  <!-- 左侧：基本信息和OAuth2授权 -->
                  <div class="form-section gmail-left">
                    <h4 class="section-title">
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                        <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"></path>
                        <circle cx="12" cy="7" r="4"></circle>
                      </svg>
                      基本信息
                    </h4>

                    <div class="form-group">
                      <label for="name">账户名称</label>
                      <input
                        id="name"
                        type="text"
                        bind:value={formData.name}
                        placeholder="例如：我的Gmail工作邮箱"
                        required
                      />
                    </div>

                    <div class="form-group">
                      <label for="email">Gmail 邮箱地址</label>
                      <input
                        id="email"
                        type="email"
                        bind:value={formData.email}
                        placeholder="your@gmail.com"
                        class:error={emailValidationError}
                        required
                      />
                      {#if emailValidationError}
                        <div class="validation-error">
                          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                            <circle cx="12" cy="12" r="10"></circle>
                            <line x1="15" y1="9" x2="9" y2="15"></line>
                            <line x1="9" y1="9" x2="15" y2="15"></line>
                          </svg>
                          {emailValidationError}
                        </div>
                      {/if}
                    </div>

                    <div class="form-group">
                      <label for="username">用户名</label>
                      <input
                        id="username"
                        type="text"
                        bind:value={formData.username}
                        placeholder="通常与邮箱地址相同"
                        required
                      />
                    </div>

                    <!-- OAuth2 授权区域 -->
                    {#if !isOAuth2Flow && !oauthToken}
                      <div class="oauth-section">
                        <div class="oauth-buttons">
                          <button type="button" class="oauth-btn" on:click={startGmailOAuth} disabled={isLoading}>
                            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                              <rect x="3" y="11" width="18" height="11" rx="2" ry="2"></rect>
                              <path d="M7 11V7a5 5 0 0 1 10 0v4"></path>
                            </svg>
                            {isLoading ? '正在授权...' : 'Gmail OAuth2 授权'}
                          </button>
                          {#if isLoading}
                            <button type="button" class="cancel-oauth-btn" on:click={cancelOAuth}>
                              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                <line x1="18" y1="6" x2="6" y2="18"></line>
                                <line x1="6" y1="6" x2="18" y2="18"></line>
                              </svg>
                              取消
                            </button>
                          {/if}
                        </div>
                        <p class="oauth-hint">
                          {#if isLoading}
                            请在浏览器中完成授权，或点击取消按钮停止授权
                          {:else}
                            点击按钮在浏览器中完成Google账户授权
                          {/if}
                        </p>
                      </div>
                    {:else if oauthToken}
                      <div class="oauth-success">
                        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                          <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"></path>
                          <polyline points="22,4 12,14.01 9,11.01"></polyline>
                        </svg>
                        <span>OAuth2 授权成功</span>
                      </div>
                    {/if}
                  </div>

                  <!-- 右侧：Gmail特色信息卡片 -->
                  <div class="form-section gmail-right">
                    <div class="gmail-info-card">
                      <div class="gmail-card-header">
                        <div class="gmail-logo">
                          <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                            <path d="M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z"></path>
                            <polyline points="22,6 12,13 2,6"></polyline>
                          </svg>
                        </div>
                        <h4>Gmail 配置</h4>
                      </div>

                      <div class="gmail-features">
                        <div class="feature-item">
                          <div class="feature-icon">🔒</div>
                          <div class="feature-text">
                            <strong>OAuth2 安全认证</strong>
                            <span>无需输入密码，更安全</span>
                          </div>
                        </div>

                        <div class="feature-item">
                          <div class="feature-icon">⚙️</div>
                          <div class="feature-text">
                            <strong>自动配置</strong>
                            <span>服务器设置自动完成</span>
                          </div>
                        </div>

                        <div class="feature-item">
                          <div class="feature-icon">📧</div>
                          <div class="feature-text">
                            <strong>Gmail API</strong>
                            <span>使用官方API，更稳定</span>
                          </div>
                        </div>
                      </div>

                      <div class="server-info">
                        <h5>服务器配置</h5>
                        <div class="server-details">
                          <div class="server-item">
                            <span class="server-label">IMAP:</span>
                            <span class="server-value">imap.gmail.com:993</span>
                          </div>
                          <div class="server-item">
                            <span class="server-label">SMTP:</span>
                            <span class="server-value">smtp.gmail.com:587</span>
                          </div>
                        </div>
                      </div>
                    </div>
                  </div>
                </div>
              {:else}
                <div class="form-layout">
                  <!-- 左侧：基本信息 -->
                  <div class="form-section">
                    <h4 class="section-title">
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                        <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"></path>
                        <circle cx="12" cy="7" r="4"></circle>
                      </svg>
                      基本信息
                    </h4>

                    <div class="form-group">
                      <label for="name">账户名称</label>
                      <input
                        id="name"
                        type="text"
                        bind:value={formData.name}
                        placeholder="例如：我的工作邮箱"
                        required
                      />
                    </div>

                  <div class="form-group">
                    <label for="email">邮箱地址</label>
                    <input
                      id="email"
                      type="email"
                      bind:value={formData.email}
                      placeholder={getEmailPlaceholder()}
                      class:error={emailValidationError}
                      required
                    />
                    {#if emailValidationError}
                      <div class="validation-error">
                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                          <circle cx="12" cy="12" r="10"></circle>
                          <line x1="15" y1="9" x2="9" y2="15"></line>
                          <line x1="9" y1="9" x2="15" y2="15"></line>
                        </svg>
                        {emailValidationError}
                      </div>
                    {/if}
                  </div>

                  <div class="form-group">
                    <label for="username">用户名</label>
                    <input
                      id="username"
                      type="text"
                      bind:value={formData.username}
                      placeholder="通常与邮箱地址相同"
                      required
                    />
                  </div>

                  {#if selectedProvider?.name === 'Gmail'}
                    <!-- Gmail OAuth2 授权 -->
                    {#if !isOAuth2Flow && !oauthToken}
                      <div class="oauth-section">
                        <div class="oauth-buttons">
                          <button type="button" class="oauth-btn" on:click={startGmailOAuth} disabled={isLoading}>
                            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                              <rect x="3" y="11" width="18" height="11" rx="2" ry="2"></rect>
                              <path d="M7 11V7a5 5 0 0 1 10 0v4"></path>
                            </svg>
                            {isLoading ? '正在授权...' : 'Gmail OAuth2 授权'}
                          </button>
                          {#if isLoading}
                            <button type="button" class="cancel-oauth-btn" on:click={cancelOAuth}>
                              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                <line x1="18" y1="6" x2="6" y2="18"></line>
                                <line x1="6" y1="6" x2="18" y2="18"></line>
                              </svg>
                              取消
                            </button>
                          {/if}
                        </div>
                        <p class="oauth-hint">
                          {#if isLoading}
                            请在浏览器中完成授权，或点击取消按钮停止授权
                          {:else}
                            点击按钮在浏览器中完成Google账户授权
                          {/if}
                        </p>
                      </div>

                    {:else if oauthToken}
                      <div class="oauth-success">
                        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                          <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"></path>
                          <polyline points="22,4 12,14.01 9,11.01"></polyline>
                        </svg>
                        <span>OAuth2 授权成功</span>
                      </div>
                    {/if}
                  {:else}
                    <!-- 传统密码输入 -->
                    <div class="form-group">
                      <label for="password">密码</label>
                      <input
                        id="password"
                        type="password"
                        bind:value={formData.password}
                        placeholder={getPasswordPlaceholder()}
                        required
                      />
                    </div>
                  {/if}
                </div>

                <!-- 右侧：服务器设置（仅对非Gmail账户显示） -->
                {#if !isGmail}
                  <div class="form-section">
                    <h4 class="section-title">
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                        <rect x="2" y="3" width="20" height="14" rx="2" ry="2"></rect>
                        <line x1="8" y1="21" x2="16" y2="21"></line>
                        <line x1="12" y1="17" x2="12" y2="21"></line>
                      </svg>
                      服务器设置
                    </h4>

                    <div class="form-row">
                      <div class="form-group">
                        <label for="imap_server">IMAP服务器</label>
                        <input
                          id="imap_server"
                          type="text"
                          bind:value={formData.imap_server}
                          placeholder={getImapPlaceholder()}
                          required
                        />
                      </div>
                      <div class="form-group">
                        <label for="imap_port">IMAP端口</label>
                        <input
                          id="imap_port"
                          type="number"
                          bind:value={formData.imap_port}
                          min="1"
                          max="65535"
                          required
                        />
                      </div>
                    </div>

                    <div class="form-row">
                      <div class="form-group">
                        <label for="smtp_server">SMTP服务器</label>
                        <input
                          id="smtp_server"
                          type="text"
                          bind:value={formData.smtp_server}
                          placeholder={getSmtpPlaceholder()}
                          required
                        />
                      </div>
                      <div class="form-group">
                        <label for="smtp_port">SMTP端口</label>
                        <input
                          id="smtp_port"
                          type="number"
                          bind:value={formData.smtp_port}
                          min="1"
                          max="65535"
                          required
                        />
                      </div>
                    </div>

                    <div class="form-group">
                      <label class="checkbox-label">
                        <input
                          type="checkbox"
                          bind:checked={formData.use_tls}
                        />
                        使用TLS加密连接
                      </label>
                    </div>

                    <!-- 按钮放在服务器设置卡片内 -->
                    <div class="form-actions">
                      <button type="button" class="cancel-btn" on:click={closeDialog}>
                        取消
                      </button>
                      <button type="submit" class="submit-btn" disabled={isLoading}>
                        {isLoading ? '测试中...' : '测试连接'}
                      </button>
                    </div>
                  </div>
                {/if}
                </div>
              {/if}
            </form>
          </div>

        {:else if step === 3}
          <!-- 测试连接 -->
          <div class="step-content">
            <div class="testing-state">
              <div class="loading-spinner">
                <div class="spinner-ring"></div>
                <div class="spinner-dot"></div>
              </div>
              <h3>正在测试连接...</h3>
              <p>请稍候，我们正在验证您的账户设置</p>
            </div>
          </div>

        {:else if step === 4}
          <!-- 测试成功 -->
          <div class="step-content">
            <div class="success-state">
              <div class="success-icon">✓</div>
              <h3>连接测试成功！</h3>
              <p>您的邮箱账户配置正确，可以正常收发邮件</p>

              <div class="account-summary">
                <h4>账户信息</h4>
                <div class="summary-item">
                  <span class="label">账户名称:</span>
                  <span class="value">{formData.name}</span>
                </div>
                <div class="summary-item">
                  <span class="label">邮箱地址:</span>
                  <span class="value">{formData.email}</span>
                </div>
                <div class="summary-item">
                  <span class="label">IMAP服务器:</span>
                  <span class="value">{formData.imap_server}:{formData.imap_port}</span>
                </div>
                <div class="summary-item">
                  <span class="label">SMTP服务器:</span>
                  <span class="value">{formData.smtp_server}:{formData.smtp_port}</span>
                </div>
              </div>

              <div class="success-actions">
                <button type="button" class="cancel-btn" on:click={closeDialog}>
                  稍后添加
                </button>
                <button type="button" class="submit-btn" on:click={saveAccount} disabled={isLoading}>
                  {isLoading ? '保存中...' : '保存账户'}
                </button>
              </div>
            </div>
          </div>

        {:else if step === 5}
          <!-- 测试失败 -->
          <div class="step-content error-step">
            <div class="error-state">
              <div class="error-icon">✗</div>
              <h3>连接测试失败</h3>
              <p>无法连接到邮箱服务器，请检查以下设置：</p>

              <div class="error-details">
                <div class="error-message">
                  <strong>问题原因：</strong>
                  <span class="error-text">{getUserFriendlyError(testError)}</span>
                </div>

                <details class="technical-details">
                  <summary>查看技术详情</summary>
                  <div class="technical-error">
                    {testError}
                  </div>
                </details>
              </div>

              <div class="troubleshooting">
                <h4>常见解决方案：</h4>
                <ul>
                  <li>检查邮箱地址和密码是否正确</li>
                  <li>确认IMAP/SMTP服务器地址和端口号</li>
                  <li>检查网络连接是否正常</li>
                  <li>确认邮箱服务商是否开启了IMAP/SMTP服务</li>
                  <li>如果使用QQ邮箱等，请使用授权码而非登录密码</li>
                </ul>
              </div>

              <div class="error-actions">
                <button type="button" class="cancel-btn" on:click={closeDialog}>
                  取消
                </button>
                <button type="button" class="submit-btn" on:click={() => step = 2}>
                  重新配置
                </button>
              </div>
            </div>
          </div>

        {:else if step === 6}
          <!-- Gmail OAuth2 授权步骤 -->
          <div class="step-content">
            <div class="gmail-oauth-container">
              <div class="gmail-oauth-header">
                <div class="gmail-icon-large">
                  <!-- 使用本地Gmail图标 -->
                  <img src="/guge.png" alt="Gmail" width="48" height="48" />
                </div>
                <h3>Gmail 账户授权</h3>
                <p>使用 Google 账户安全登录，无需输入密码</p>
              </div>

              <div class="gmail-oauth-form">
                <div class="form-group">
                  <label for="gmail-name">账户名称</label>
                  <input
                    id="gmail-name"
                    type="text"
                    bind:value={formData.name}
                    placeholder="例如：我的Gmail工作邮箱"
                    required
                  />
                </div>

                <div class="form-group">
                  <label for="gmail-email">Gmail 邮箱地址</label>
                  <input
                    id="gmail-email"
                    type="email"
                    bind:value={formData.email}
                    placeholder="your@gmail.com"
                    required
                  />
                </div>

                {#if !oauthToken}
                  <div class="oauth-action">
                    <button type="button" class="gmail-oauth-btn" on:click={startGmailOAuth} disabled={isLoading}>
                      <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                        <rect x="3" y="11" width="18" height="11" rx="2" ry="2"></rect>
                        <path d="M7 11V7a5 5 0 0 1 10 0v4"></path>
                      </svg>
                      {isLoading ? '正在授权...' : '开始 Google 授权'}
                    </button>
                    {#if isLoading}
                      <button type="button" class="cancel-oauth-btn" on:click={cancelOAuth}>
                        取消授权
                      </button>
                    {/if}
                    <p class="oauth-hint">
                      {#if isLoading}
                        请在弹出的浏览器窗口中完成 Google 账户授权
                      {:else}
                        点击按钮将打开安全的 Google 授权页面
                      {/if}
                    </p>
                  </div>
                {:else}
                  <div class="oauth-success-simple">
                    <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"></path>
                      <polyline points="22,4 12,14.01 9,11.01"></polyline>
                    </svg>
                    <span>授权成功！Gmail 账户已连接</span>
                  </div>
                {/if}
              </div>


            </div>
          </div>



        {:else if step === 7}
          <!-- Outlook OAuth2 授权步骤 -->
          <div class="step-content">
            <div class="outlook-oauth-container">
              <div class="outlook-oauth-header">
                <div class="outlook-icon-large">
                  <!-- 使用本地Outlook图标 -->
                  <img src="/Outlook.png" alt="Outlook" width="48" height="48" />
                </div>
                <h3>Outlook 账户授权</h3>
                <p>使用 Microsoft 账户安全登录，无需输入密码</p>
              </div>

              <div class="outlook-oauth-form">
                <div class="form-group">
                  <label for="outlook-name">账户名称</label>
                  <input
                    id="outlook-name"
                    type="text"
                    bind:value={formData.name}
                    placeholder="例如：我的Outlook工作邮箱"
                    required
                  />
                </div>

                <div class="form-group">
                  <label for="outlook-email">Outlook 邮箱地址</label>
                  <input
                    id="outlook-email"
                    type="email"
                    bind:value={formData.email}
                    placeholder="your@outlook.com"
                    required
                  />
                </div>

                {#if !oauthToken}
                  <div class="oauth-action">
                    <button type="button" class="outlook-oauth-btn" on:click={startOutlookOAuth} disabled={isLoading}>
                      <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                        <rect x="3" y="11" width="18" height="11" rx="2" ry="2"></rect>
                        <path d="M7 11V7a5 5 0 0 1 10 0v4"></path>
                      </svg>
                      {isLoading ? '正在授权...' : '开始 Microsoft 授权'}
                    </button>
                    {#if isLoading}
                      <button type="button" class="cancel-oauth-btn" on:click={cancelOAuth}>
                        取消授权
                      </button>
                    {/if}
                    <p class="oauth-hint">
                      {#if isLoading}
                        请在弹出的浏览器窗口中完成 Microsoft 账户授权
                      {:else}
                        点击按钮将打开安全的 Microsoft 授权页面
                      {/if}
                    </p>
                  </div>
                {:else}
                  <div class="oauth-success-simple">
                    <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"></path>
                      <polyline points="22,4 12,14.01 9,11.01"></polyline>
                    </svg>
                    <span>授权成功！Outlook 账户已连接</span>
                  </div>
                {/if}
              </div>


            </div>
          </div>
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
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
    background: linear-gradient(135deg, rgba(102, 126, 234, 0.15) 0%, rgba(118, 75, 162, 0.15) 100%);
    backdrop-filter: blur(12px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
    animation: fadeIn 0.3s ease-out;
  }

  @keyframes fadeIn {
    from {
      opacity: 0;
      backdrop-filter: blur(0px);
    }
    to {
      opacity: 1;
      backdrop-filter: blur(12px);
    }
  }

  .dialog {
    background: white;
    border-radius: 16px;
    box-shadow: 0 25px 50px rgba(0, 0, 0, 0.2);
    width: 95%;
    max-width: 1000px;
    min-height: 600px;
    max-height: 85vh;
    overflow: visible;
    display: flex;
    flex-direction: column;
    position: relative;
    backdrop-filter: blur(10px);
    animation: slideUp 0.4s cubic-bezier(0.25, 0.46, 0.45, 0.94);
  }

  @keyframes slideUp {
    from {
      opacity: 0;
      transform: translateY(30px) scale(0.95);
    }
    to {
      opacity: 1;
      transform: translateY(0) scale(1);
    }
  }

  .dialog-header {
    padding: 12px 32px 8px 32px;
    display: flex;
    justify-content: space-between;
    align-items: center;
    border-bottom: 1px solid rgba(233, 236, 239, 0.6);
    background: linear-gradient(135deg, #f8f9fa 0%, #ffffff 100%);
    position: relative;
    border-radius: 16px 16px 0 0;
  }

  .gmail-header-minimal {
    background: transparent !important;
    border-bottom: none !important;
    padding: 12px 16px !important;
    position: absolute;
    top: 0;
    right: 0;
    z-index: 10;
    border-radius: 0 16px 0 0 !important;
  }

  .gmail-header-minimal .close-btn {
    background: rgba(255, 255, 255, 0.9);
    color: #5f6368;
    border: 1px solid rgba(0, 0, 0, 0.1);
    backdrop-filter: blur(10px);
  }

  .gmail-header-minimal .close-btn:hover {
    background: rgba(255, 255, 255, 1);
    color: #3c4043;
    border-color: rgba(0, 0, 0, 0.2);
  }

  .dialog-header h2 {
    margin: 0;
    color: var(--theme-primary);
    font-size: 1.3rem;
    font-weight: 700;
    background: linear-gradient(135deg, var(--theme-primary), var(--theme-secondary));
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
    background-clip: text;
  }

  .close-btn {
    position: absolute;
    top: -18px;
    right: -18px;
    width: 36px;
    height: 36px;
    background: rgba(255, 255, 255, 0.9);
    border: 1px solid rgba(0, 0, 0, 0.1);
    border-radius: 8px;
    cursor: pointer;
    color: #5f6368;
    font-size: 18px;
    font-weight: 400;
    display: flex;
    align-items: center;
    justify-content: center;
    backdrop-filter: blur(10px);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
    z-index: 1001;
    transition: all 0.3s ease;
    line-height: 1;
    text-align: center;
  }

  .close-btn:hover {
    background: rgba(255, 255, 255, 1);
    color: #3c4043;
    border-color: rgba(0, 0, 0, 0.2);
    transform: scale(1.05);
  }

  .dialog-content {
    padding: 16px 32px 32px 32px;
    overflow-y: visible;
    flex: 1;
    background: #ffffff;
    border-radius: 0 0 16px 16px;
    min-height: 0;
  }

  /* Gmail对话框样式 - 与Outlook保持一致的宽度和高度 */
  .gmail-dialog {
    max-width: 600px;
    width: 90vw;
    height: auto;
    max-height: 80vh;
    background: linear-gradient(135deg, #f8f9fa 0%, #ffffff 100%);
    position: relative;
    border-radius: 16px;
  }

  /* Gmail步骤的特殊样式 - 保持顶部圆角 */
  .gmail-dialog .dialog-content {
    border-radius: 16px;
    padding: 24px 32px 24px 32px; /* 减少内边距，为浮动的关闭按钮留出空间 */
    overflow-y: visible;
  }

  .step-content h3 {
    margin: 0 0 20px 0;
    color: #495057;
    font-size: 1.2rem;
  }



  .provider-grid {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 24px;
    max-height: 600px;
    overflow-y: auto;
    padding: 16px 16px 16px 8px;
    margin: -8px;
  }

  @media (max-width: 1024px) {
    .provider-grid {
      grid-template-columns: repeat(3, 1fr);
    }
  }

  @media (max-width: 768px) {
    .provider-grid {
      grid-template-columns: repeat(2, 1fr);
    }
  }

  @media (max-width: 480px) {
    .provider-grid {
      grid-template-columns: 1fr;
    }
  }

  .provider-grid::-webkit-scrollbar {
    width: 6px;
  }

  .provider-grid::-webkit-scrollbar-track {
    background: #f1f1f1;
    border-radius: 3px;
  }

  .provider-grid::-webkit-scrollbar-thumb {
    background: #c1c1c1;
    border-radius: 3px;
  }

  .provider-grid::-webkit-scrollbar-thumb:hover {
    background: #a8a8a8;
  }

  /* 对话框内容区域滚动条样式 */
  .dialog-content::-webkit-scrollbar {
    width: 6px;
  }

  .dialog-content::-webkit-scrollbar-track {
    background: transparent;
  }

  .dialog-content::-webkit-scrollbar-thumb {
    background: #c1c1c1;
    border-radius: 3px;
  }

  .dialog-content::-webkit-scrollbar-thumb:hover {
    background: #a8a8a8;
  }

  .provider-card {
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 24px 20px;
    border: 2px solid rgba(233, 236, 239, 0.6);
    border-radius: 16px;
    background: linear-gradient(135deg, #ffffff 0%, #f8f9fa 100%);
    cursor: pointer;
    text-align: center;
    transition: all 0.4s cubic-bezier(0.25, 0.46, 0.45, 0.94);
    position: relative;
    min-height: 160px;
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.08);
  }



  .provider-card:hover {
    border-color: #667eea;
    background: linear-gradient(135deg, #f8f9ff 0%, #ffffff 100%);
    transform: translateY(-2px) scale(1.01);
    box-shadow: 0 8px 16px rgba(0, 0, 0, 0.1);
    z-index: 10;
  }

  .provider-card:active {
    transform: translateY(-1px) scale(1.005);
  }

  .custom-card {
    border-style: dashed;
    border-color: #6c757d;
  }

  .custom-card:hover {
    border-color: #495057;
    background: #f8f9fa;
  }

  .provider-icon {
    width: 56px;
    height: 56px;
    margin-bottom: 16px;
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 16px;
    background: linear-gradient(135deg, #f8f9fa 0%, #ffffff 100%);
    flex-shrink: 0;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
    transition: all 0.3s ease;
  }

  .provider-card:hover .provider-icon {
    transform: scale(1.1);
    box-shadow: 0 8px 20px rgba(102, 126, 234, 0.2);
  }

  .provider-icon img {
    width: 36px;
    height: 36px;
    object-fit: contain;
    border-radius: 8px;
  }

  .provider-fallback {
    font-size: 18px;
    font-weight: 600;
    color: #495057;
    width: 32px;
    height: 32px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: #e9ecef;
    border-radius: 4px;
  }

  .custom-icon {
    background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
    color: white;
  }

  .provider-info {
    flex: 1;
  }

  .provider-name {
    font-weight: 700;
    color: var(--theme-primary);
    margin-bottom: 8px;
    font-size: 16px;
    background: linear-gradient(135deg, var(--theme-primary), var(--theme-secondary));
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
    background-clip: text;
  }

  .provider-details {
    font-size: 13px;
    color: #6c757d;
    margin-bottom: 12px;
    line-height: 1.4;
    opacity: 0.9;
  }

  .provider-config {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin-top: 8px;
  }

  .config-item {
    font-size: 10px;
    color: #6c757d;
    font-family: 'SF Mono', 'Monaco', 'Inconsolata', 'Roboto Mono', monospace;
    background: linear-gradient(135deg, rgba(108, 122, 224, 0.1) 0%, rgba(123, 104, 238, 0.1) 100%);
    padding: 4px 8px;
    border-radius: 8px;
    display: inline-block;
    text-align: center;
    border: 1px solid rgba(233, 236, 239, 0.8);
    font-weight: 500;
  }

  .provider-arrow {
    font-size: 16px;
    color: #6c757d;
    margin-top: 8px;
    transition: transform 0.3s ease;
    position: absolute;
    bottom: 12px;
    right: 12px;
  }

  .provider-card:hover .provider-arrow {
    transform: translateX(2px);
    color: #667eea;
  }

  .header-with-icon {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .provider-header-icon {
    width: 32px;
    height: 32px;
    object-fit: contain;
    transition: all 0.2s ease;
  }

  .provider-header-icon:hover {
    transform: scale(1.05);
  }

  .default-email-icon {
    width: 32px;
    height: 32px;
    color: #667eea;
    transition: all 0.2s ease;
  }

  .default-email-icon:hover {
    color: #5a67d8;
    transform: scale(1.05);
  }

  .success-icon {
    color: #28a745;
    transition: all 0.2s ease;
  }

  .success-icon:hover {
    color: #218838;
    transform: scale(1.05);
  }

  .error-icon {
    color: #dc3545;
    transition: all 0.2s ease;
  }

  .error-icon:hover {
    color: #c82333;
    transform: scale(1.05);
  }

  .account-form {
    display: flex;
    flex-direction: column;
    gap: 24px;
    min-height: 450px;
    justify-content: flex-start;
  }

  .form-layout {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 32px;
  }

  @media (max-width: 768px) {
    .form-layout {
      grid-template-columns: 1fr;
      gap: 24px;
    }
  }

  .form-section {
    background: linear-gradient(135deg, #ffffff 0%, #f8f9fa 100%);
    padding: 28px;
    border-radius: 16px;
    border: 1px solid rgba(233, 236, 239, 0.6);
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.08);
    display: flex;
    flex-direction: column;
    gap: 20px;
    position: relative;
    overflow: hidden;
    transition: all 0.3s ease;
  }





  @keyframes fadeInUp {
    from {
      opacity: 0;
      transform: translateY(20px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  .form-group {
    animation: fadeInUp 0.5s ease-out;
  }

  .form-group:nth-child(2) { animation-delay: 0.1s; }
  .form-group:nth-child(3) { animation-delay: 0.2s; }
  .form-group:nth-child(4) { animation-delay: 0.3s; }
  .form-group:nth-child(5) { animation-delay: 0.4s; }

  .section-title {
    display: flex;
    align-items: center;
    gap: 12px;
    margin: 0 0 24px 0;
    color: #2c3e50;
    font-size: 1.1rem;
    font-weight: 700;
    padding-bottom: 16px;
    border-bottom: 2px solid #e9ecef;
  }

  .section-title svg {
    color: #667eea;
  }

  .form-group {
    display: flex;
    flex-direction: column;
    gap: 8px;
    position: relative;
  }

  .form-group label {
    font-weight: 600;
    color: #2c3e50;
    font-size: 14px;
    margin-left: 4px;
    transition: all 0.2s ease;
  }

  .form-group input {
    padding: 14px 20px;
    border: 2px solid #e9ecef;
    border-radius: 12px;
    font-size: 14px;
    transition: all 0.3s ease;
    background: linear-gradient(135deg, #ffffff 0%, #f8f9fa 100%);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.04);
    position: relative;
  }

  .form-group input:hover {
    border-color: #667eea;
    transform: translateY(-1px);
    box-shadow: 0 4px 12px rgba(102, 126, 234, 0.15);
  }

  .form-group input:focus {
    outline: none;
    border-color: #667eea;
    background: #ffffff;
    box-shadow: 0 0 0 4px rgba(102, 126, 234, 0.15), 0 4px 20px rgba(102, 126, 234, 0.1);
    transform: translateY(-2px);
  }

  .form-group:focus-within label {
    color: #667eea;
    transform: translateY(-2px);
  }

  .form-group input::placeholder {
    color: #adb5bd;
    font-style: italic;
  }

  .form-group input.error {
    border-color: #dc3545;
    background-color: #fff5f5;
  }

  .form-group input.error:focus {
    border-color: #dc3545;
    box-shadow: 0 0 0 3px rgba(220, 53, 69, 0.1);
  }

  .validation-error {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 6px;
    padding: 8px 12px;
    background: #fff5f5;
    border: 1px solid #fecaca;
    border-radius: 6px;
    color: #dc3545;
    font-size: 12px;
    font-weight: 500;
  }

  .validation-error svg {
    flex-shrink: 0;
    color: #dc3545;
  }

  .form-row {
    display: grid;
    grid-template-columns: 1fr 120px;
    gap: 20px;
    align-items: end;
  }



  .checkbox-label {
    display: flex;
    align-items: center;
    gap: 12px;
    cursor: pointer;
    font-weight: normal !important;
    padding: 16px 20px;
    background: linear-gradient(135deg, #ffffff 0%, #f8f9fa 100%);
    border-radius: 12px;
    border: 2px solid #e9ecef;
    transition: all 0.3s ease;
    position: relative;
    overflow: hidden;
  }



  .checkbox-label input[type="checkbox"] {
    width: 20px;
    height: 20px;
    margin: 0;
    accent-color: #667eea;
    cursor: pointer;
  }



  /* OAuth2 相关样式 */
  .oauth-section {
    margin-bottom: 20px;
    text-align: center;
  }

  .oauth-buttons {
    display: flex;
    gap: 12px;
    justify-content: center;
    align-items: center;
    margin-bottom: 12px;
  }

  .oauth-btn {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 12px 24px;
    background: linear-gradient(135deg, #4285f4, #34a853);
    color: white;
    border: none;
    border-radius: 12px;
    font-size: 1rem;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.3s ease;
    box-shadow: 0 4px 12px rgba(66, 133, 244, 0.3);
  }

  .oauth-btn:hover {
    transform: translateY(-2px);
    box-shadow: 0 6px 16px rgba(66, 133, 244, 0.4);
  }

  .oauth-btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
    transform: none;
  }

  .cancel-oauth-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 8px 16px;
    background: #f8f9fa;
    color: #6c757d;
    border: 1px solid #dee2e6;
    border-radius: 6px;
    font-size: 0.9rem;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.3s ease;
  }

  .cancel-oauth-btn:hover {
    background: #e9ecef;
    color: #495057;
    border-color: #adb5bd;
  }

  .oauth-hint {
    margin-top: 8px;
    font-size: 0.85rem;
    color: #666;
    line-height: 1.4;
  }



  .oauth-success {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px 16px;
    background: linear-gradient(135deg, #e8f5e8, #d4edda);
    border: 1px solid #28a745;
    border-radius: 8px;
    color: #155724;
    font-weight: 600;
    margin-bottom: 20px;
  }

  .oauth-success svg {
    color: #28a745;
  }



  .form-actions {
    display: flex;
    justify-content: flex-end;
    gap: 12px;
    margin-top: 24px;
    padding-top: 20px;
    border-top: 1px solid #e9ecef;
  }

  .cancel-btn, .submit-btn {
    padding: 14px 28px;
    border-radius: 12px;
    font-size: 14px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.3s ease;
    position: relative;
    overflow: hidden;
    border: none;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
  }

  .cancel-btn {
    background: linear-gradient(135deg, #ffffff 0%, #f8f9fa 100%);
    color: #495057;
    border: 2px solid #e9ecef;
  }



  .submit-btn {
    background: linear-gradient(135deg, var(--theme-primary) 0%, var(--theme-secondary) 100%);
    color: white;
    position: relative;
  }





  .submit-btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
    transform: none;
  }

  /* 测试连接状态样式 */
  .testing-state {
    text-align: center;
    padding: 60px 20px;
    display: flex;
    flex-direction: column;
    justify-content: center;
    min-height: 400px;
  }

  .loading-spinner {
    position: relative;
    width: 60px;
    height: 60px;
    margin: 0 auto 20px auto;
  }

  .spinner-ring {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    border: 4px solid #e9ecef;
    border-top: 4px solid #007bff;
    border-radius: 50%;
    animation: spin 1s linear infinite;
  }

  .spinner-dot {
    position: absolute;
    top: 50%;
    left: 50%;
    width: 8px;
    height: 8px;
    background: #007bff;
    border-radius: 50%;
    transform: translate(-50%, -50%);
    animation: pulse 1.5s ease-in-out infinite;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }

  @keyframes pulse {
    0%, 100% {
      opacity: 0.3;
      transform: translate(-50%, -50%) scale(0.8);
    }
    50% {
      opacity: 1;
      transform: translate(-50%, -50%) scale(1.2);
    }
  }

  .testing-state h3 {
    margin-bottom: 10px;
  }

  .testing-state p {
    color: #6c757d;
    margin: 0;
  }

  /* 成功状态样式 */
  .success-state {
    text-align: center;
    padding: 40px 20px;
    display: flex;
    flex-direction: column;
    justify-content: center;
    min-height: 450px;
  }

  .success-icon {
    font-size: 48px;
    margin-bottom: 20px;
    color: #28a745;
    font-weight: bold;
  }

  .success-state h3 {
    color: #28a745;
    margin-bottom: 12px;
    font-size: 1.4rem;
  }

  .success-state p {
    color: #6c757d;
    margin-bottom: 32px;
    font-size: 14px;
  }

  .account-summary {
    background: #f8f9fa;
    border-radius: 12px;
    padding: 24px;
    margin-bottom: 32px;
    text-align: left;
  }

  .account-summary h4 {
    margin: 0 0 16px 0;
    color: #495057;
    font-size: 1rem;
    text-align: center;
  }

  .summary-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 8px 0;
    border-bottom: 1px solid #e9ecef;
  }

  .summary-item:last-child {
    border-bottom: none;
  }

  .summary-item .label {
    font-weight: 600;
    color: #495057;
    font-size: 14px;
  }

  .summary-item .value {
    color: #6c757d;
    font-size: 14px;
    font-family: 'Courier New', monospace;
  }

  .success-actions {
    display: flex;
    justify-content: center;
    gap: 16px;
  }

  /* 失败状态样式 */
  .error-state {
    text-align: center;
    padding: 8px 0 20px 0;
    margin: 0 -24px;
    min-height: 450px;
    display: flex;
    flex-direction: column;
    justify-content: flex-start;
  }

  .error-icon {
    font-size: 48px;
    margin: 0 0 16px 0;
    color: #dc3545;
    font-weight: bold;
  }

  .error-state h3 {
    color: #dc3545;
    margin-bottom: 8px;
    font-size: 1.4rem;
  }

  .error-state p {
    color: #6c757d;
    margin-bottom: 20px;
    font-size: 14px;
  }

  .error-details {
    background: #f8d7da;
    border: 1px solid #f5c6cb;
    border-radius: 12px;
    padding: 16px;
    margin: 0 24px 0 24px;
    text-align: left;
  }

  .error-message {
    margin-bottom: 0;
  }

  .error-message strong {
    color: #721c24;
    font-size: 14px;
  }

  .error-text {
    color: #721c24;
    font-size: 14px;
    display: block;
    margin-top: 8px;
    padding: 8px;
    background: rgba(255, 255, 255, 0.5);
    border-radius: 6px;
    line-height: 1.4;
  }

  .technical-details {
    margin-top: 12px;
    border-top: 1px solid #f5c6cb;
    padding-top: 12px;
  }

  .technical-details summary {
    color: #721c24;
    font-size: 12px;
    cursor: pointer;
    opacity: 0.8;
  }

  .technical-details summary:hover {
    opacity: 1;
  }

  .technical-error {
    margin-top: 8px;
    padding: 8px;
    background: rgba(0, 0, 0, 0.05);
    border-radius: 4px;
    font-family: 'Courier New', monospace;
    font-size: 11px;
    color: #6c757d;
    word-break: break-all;
    line-height: 1.3;
  }

  .troubleshooting {
    background: #fff3cd;
    border: 1px solid #ffeaa7;
    border-radius: 12px;
    padding: 20px;
    margin: 16px 24px 0 24px;
    text-align: left;
  }

  .troubleshooting h4 {
    margin: 0 0 12px 0;
    color: #856404;
    font-size: 14px;
  }

  .troubleshooting ul {
    margin: 0;
    padding-left: 20px;
    color: #856404;
  }

  .troubleshooting li {
    margin-bottom: 6px;
    font-size: 13px;
    line-height: 1.4;
  }

  .error-actions {
    display: flex;
    justify-content: center;
    gap: 16px;
    margin-top: 24px;
  }







  /* Gmail左右布局样式 */
  .gmail-layout {
    gap: 32px;
  }

  .gmail-left {
    flex: 1;
  }

  .gmail-right {
    flex: 1;
    display: flex;
    align-items: flex-start;
  }

  .gmail-info-card {
    width: 100%;
    background: linear-gradient(135deg, #f8f9ff 0%, #ffffff 100%);
    border: 2px solid #e3f2fd;
    border-radius: 16px;
    padding: 24px;
    box-shadow: 0 8px 32px rgba(66, 133, 244, 0.1);
  }

  .gmail-card-header {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 24px;
    padding-bottom: 16px;
    border-bottom: 2px solid #e8f4fd;
  }

  .gmail-logo {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 48px;
    height: 48px;
    background: linear-gradient(135deg, #4285f4, #34a853);
    border-radius: 12px;
    color: white;
  }

  .gmail-card-header h4 {
    margin: 0;
    font-size: 1.3rem;
    font-weight: 700;
    color: #1a73e8;
  }

  .gmail-features {
    margin-bottom: 24px;
  }

  .feature-item {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 0;
    border-bottom: 1px solid #f0f0f0;
  }

  .feature-item:last-child {
    border-bottom: none;
  }

  .feature-icon {
    font-size: 1.2rem;
    width: 32px;
    text-align: center;
  }

  .feature-text {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .feature-text strong {
    font-size: 0.95rem;
    font-weight: 600;
    color: #1a73e8;
  }

  .feature-text span {
    font-size: 0.85rem;
    color: #5f6368;
  }

  .server-info {
    background: linear-gradient(135deg, #e8f5e8 0%, #f0f8f0 100%);
    border: 1px solid #c8e6c9;
    border-radius: 12px;
    padding: 16px;
  }

  .server-info h5 {
    margin: 0 0 12px 0;
    font-size: 1rem;
    font-weight: 600;
    color: #2e7d32;
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .server-info h5::before {
    content: "⚙️";
    font-size: 1.1rem;
  }

  .server-details {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .server-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 6px 0;
  }

  .server-label {
    font-weight: 600;
    color: #2e7d32;
    font-size: 0.9rem;
  }

  .server-value {
    font-family: 'Courier New', monospace;
    font-size: 0.85rem;
    color: #388e3c;
    background: rgba(76, 175, 80, 0.1);
    padding: 2px 8px;
    border-radius: 4px;
  }

  /* Gmail简化授权页面样式 */
  .gmail-oauth-container {
    max-width: 500px;
    margin: 0 auto;
    text-align: center;
  }

  .gmail-oauth-header {
    margin-bottom: 32px;
  }

  .gmail-icon-large {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    margin-bottom: 24px;
  }

  .gmail-icon-large img {
    width: 80px;
    height: 80px;
    object-fit: contain;
  }

  .gmail-oauth-header h3 {
    margin: 0 0 12px 0;
    font-size: 1.8rem;
    font-weight: 700;
    color: #1a73e8;
  }

  .gmail-oauth-header p {
    margin: 0;
    font-size: 1rem;
    color: #5f6368;
    line-height: 1.5;
  }

  .gmail-oauth-form {
    background: #f8f9fa;
    border-radius: 16px;
    padding: 24px;
    margin-bottom: 16px;
    text-align: left;
  }

  .oauth-action {
    text-align: center;
    margin-top: 20px;
  }

  .gmail-oauth-btn {
    display: inline-flex;
    align-items: center;
    gap: 12px;
    padding: 16px 32px;
    background: linear-gradient(135deg, #4285f4, #34a853);
    color: white;
    border: none;
    border-radius: 12px;
    font-size: 1.1rem;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.3s ease;
    box-shadow: 0 4px 16px rgba(66, 133, 244, 0.3);
    margin-bottom: 16px;
  }

  .gmail-oauth-btn:hover:not(:disabled) {
    transform: translateY(-2px);
    box-shadow: 0 6px 20px rgba(66, 133, 244, 0.4);
  }

  .gmail-oauth-btn:disabled {
    opacity: 0.7;
    cursor: not-allowed;
    transform: none;
  }

  .oauth-success-simple {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 12px;
    padding: 20px;
    background: linear-gradient(135deg, #e8f5e8, #d4edda);
    border: 2px solid #28a745;
    border-radius: 12px;
    color: #155724;
    font-weight: 600;
    margin-top: 24px;
  }

  .oauth-success-simple svg {
    color: #28a745;
  }



  /* Outlook OAuth2 样式 - 与Gmail保持一致 */
  .outlook-dialog {
    max-width: 600px;
    width: 90vw;
    height: auto;
    max-height: 80vh;
    background: linear-gradient(135deg, #f8f9fa 0%, #ffffff 100%);
    position: relative;
    border-radius: 16px;
  }

  .outlook-header-minimal {
    background: transparent !important;
    border-bottom: none !important;
    padding: 12px 16px !important;
    position: absolute;
    top: 0;
    right: 0;
    z-index: 10;
    border-radius: 0 16px 0 0 !important;
  }

  .outlook-header-minimal .close-btn {
    background: rgba(255, 255, 255, 0.9);
    color: #5f6368;
    border: 1px solid rgba(0, 0, 0, 0.1);
    backdrop-filter: blur(10px);
  }

  .outlook-header-minimal .close-btn:hover {
    background: rgba(255, 255, 255, 1);
    color: #3c4043;
    border-color: rgba(0, 0, 0, 0.2);
  }

  /* Outlook步骤的特殊样式 - 保持顶部圆角 */
  .outlook-dialog .dialog-content {
    border-radius: 16px;
    padding: 24px 32px 24px 32px; /* 减少内边距，为浮动的关闭按钮留出空间 */
    overflow-y: visible;
  }

  /* Outlook OAuth2 授权页面样式 */
  .outlook-oauth-container {
    max-width: 500px;
    margin: 0 auto;
    text-align: center;
  }

  .outlook-oauth-header {
    margin-bottom: 32px;
  }

  .outlook-icon-large {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    margin-bottom: 24px;
  }

  .outlook-icon-large img {
    width: 80px;
    height: 80px;
    object-fit: contain;
  }

  .outlook-oauth-header h3 {
    margin: 0 0 12px 0;
    font-size: 1.8rem;
    font-weight: 700;
    color: #0078d4;
  }

  .outlook-oauth-header p {
    margin: 0;
    font-size: 1rem;
    color: #5f6368;
    line-height: 1.5;
  }

  .outlook-oauth-form {
    background: #f8f9fa;
    border-radius: 16px;
    padding: 24px;
    margin-bottom: 16px;
    text-align: left;
  }

  /* Outlook表单组样式 - 与Gmail保持一致 */
  .outlook-oauth-form .form-group {
    margin-bottom: 16px;
  }

  .outlook-oauth-form .form-group label {
    display: block;
    margin-bottom: 8px;
    font-weight: 600;
    color: #3c4043;
    font-size: 0.95rem;
  }

  .outlook-oauth-form .form-group input {
    width: 100%;
    padding: 16px 20px;
    border: 2px solid #e8eaed;
    border-radius: 12px;
    font-size: 1rem;
    background: white;
    transition: all 0.3s ease;
    box-sizing: border-box;
  }

  .outlook-oauth-form .form-group input:focus {
    outline: none;
    border-color: #0078d4;
    box-shadow: 0 0 0 3px rgba(0, 120, 212, 0.1);
  }

  .outlook-oauth-form .form-group input::placeholder {
    color: #9aa0a6;
  }

  /* OAuth操作区域样式 */
  .outlook-oauth-form .oauth-action {
    text-align: center;
    margin-top: 20px;
  }

  .outlook-oauth-form .oauth-hint {
    margin: 16px 0 0 0;
    font-size: 0.9rem;
    color: #5f6368;
    line-height: 1.4;
  }

  .outlook-oauth-form .cancel-oauth-btn {
    display: inline-block;
    margin-top: 12px;
    padding: 8px 16px;
    background: transparent;
    color: #5f6368;
    border: 1px solid #dadce0;
    border-radius: 8px;
    font-size: 0.9rem;
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .outlook-oauth-form .cancel-oauth-btn:hover {
    background: #f8f9fa;
    border-color: #5f6368;
  }

  /* OAuth成功状态样式 */
  .outlook-oauth-form .oauth-success-simple {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 12px;
    padding: 20px;
    background: linear-gradient(135deg, #e8f5e8, #f0f8f0);
    border: 2px solid #34a853;
    border-radius: 12px;
    color: #137333;
    font-weight: 600;
    margin-top: 24px;
  }

  .outlook-oauth-form .oauth-success-simple svg {
    color: #34a853;
    flex-shrink: 0;
  }

  .outlook-oauth-btn {
    display: inline-flex;
    align-items: center;
    gap: 12px;
    padding: 16px 32px;
    background: linear-gradient(135deg, #0078d4, #106ebe);
    color: white;
    border: none;
    border-radius: 12px;
    font-size: 1rem;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.3s ease;
    box-shadow: 0 4px 16px rgba(0, 120, 212, 0.3);
    margin-bottom: 16px;
  }

  .outlook-oauth-btn:hover:not(:disabled) {
    transform: translateY(-2px);
    box-shadow: 0 6px 20px rgba(0, 120, 212, 0.4);
  }

  .outlook-oauth-btn:disabled {
    opacity: 0.7;
    cursor: not-allowed;
    transform: none;
  }


</style>
