<script lang="ts">
  import { onMount } from 'svelte';
  import { EmailAPI } from '../lib/api';
  import { accounts, currentAccount, addNotification, currentUser } from '../lib/store';
  import type { UserInfo } from '../lib/store';

  import InnovativeEmailInterface from '../lib/components/InnovativeEmailInterface.svelte';
  import AddAccountDialog from '../lib/components/AddAccountDialog.svelte';
  import Notifications from '../lib/components/Notifications.svelte';
  import SplashScreen from '../lib/components/SplashScreen.svelte';
  import AuthScreen from '../lib/components/AuthScreen.svelte';

  let showSplash = true;
  let showAuth = false;
  let isLoggedIn = false;
  let appInitialized = false;

  onMount(async () => {
    // 检查是否有保存的用户登录状态
    await checkSavedLoginState();

    // 监听自定义协议回调（QQ登录）
    if (typeof window !== 'undefined') {
      // 检查是否通过自定义协议启动
      const urlParams = new URLSearchParams(window.location.search);
      const protocolData = urlParams.get('protocol');

      if (protocolData) {
        console.log('检测到自定义协议启动:', protocolData);
        // 处理QQ登录回调
        handleQQProtocolCallback(protocolData);
      }
    }
  });

  // 检查保存的登录状态
  async function checkSavedLoginState() {
    try {
      console.log('检查保存的用户登录状态...');
      const savedUser = localStorage.getItem('currentUser');
      if (savedUser) {
        const userInfo: UserInfo = JSON.parse(savedUser);
        console.log('✅ 发现保存的用户登录状态:', userInfo);

        // 恢复用户状态（但保持开屏显示）
        currentUser.set(userInfo);
        isLoggedIn = true;
        // 不修改 showSplash 和 showAuth，让开屏正常播放

        // 初始化应用
        try {
          await EmailAPI.initDatabase();
          const accountList = await EmailAPI.getAccounts();
          accounts.set(accountList);

          if (accountList.length > 0) {
            currentAccount.set(accountList[0]);
          }

          appInitialized = true;
          console.log('✅ 应用初始化完成（从保存状态恢复）');
          addNotification('success', `欢迎回来，${userInfo.username}！`);
        } catch (error) {
          console.error('从保存状态恢复应用初始化失败:', error);
          // 如果初始化失败，清除保存的状态，让开屏结束后显示登录界面
          localStorage.removeItem('currentUser');
          currentUser.set(null);
          isLoggedIn = false;
          addNotification('error', '恢复登录状态失败，请重新登录');
        }
      } else {
        console.log('❌ 没有找到保存的用户登录状态');
      }
    } catch (error) {
      console.error('❌ 检查保存的登录状态失败:', error);
      // 清除可能损坏的数据
      localStorage.removeItem('currentUser');
    }
  }

  // 处理QQ登录协议回调
  async function handleQQProtocolCallback(protocolUrl: string) {
    try {
      console.log('处理QQ协议回调:', protocolUrl);

      const { invoke } = await import('@tauri-apps/api/core');
      const result = await invoke('handle_qq_protocol_callback', { protocolUrl }) as any;

      if (result && result.success) {
        console.log('QQ登录成功:', result);

        // 模拟登录成功事件
        const loginEvent = new CustomEvent('loginSuccess', {
          detail: {
            username: result.userInfo.nickname || 'QQ用户',
            email: result.userInfo.openid + '@qq.com',
            authType: 'qq_oauth2',
            qqInfo: result.userInfo,
            user: result.user
          }
        });

        handleLoginSuccess(loginEvent);
      } else {
        console.error('QQ登录失败:', result?.message);
        addNotification('error', result?.message || 'QQ登录失败');
      }
    } catch (error) {
      console.error('处理QQ协议回调失败:', error);
      addNotification('error', 'QQ登录失败');
    }
  }

  function handleSplashComplete() {
    showSplash = false;
    // 只有在用户未登录时才显示认证界面
    if (!isLoggedIn) {
      showAuth = true;
    }
  }

  async function handleLoginSuccess(event: CustomEvent) {
    const { username, email, authType, user } = event.detail;

    console.log('handleLoginSuccess - 接收到的数据:', { username, email, authType, user });
    console.log('handleLoginSuccess - 用户头像:', user?.avatar);

    // 设置用户信息到store
    const userInfo: UserInfo = {
      id: user?.id || 'unknown',
      username: user?.username || username,
      email: user?.email || email || username,
      authType: authType || 'password',
      avatar: user?.avatar // 包含头像信息
    };

    console.log('handleLoginSuccess - 设置到store的用户信息:', userInfo);
    currentUser.set(userInfo);

    // 保存用户登录状态到 localStorage
    try {
      localStorage.setItem('currentUser', JSON.stringify(userInfo));
      console.log('用户登录状态已保存到本地存储');
    } catch (error) {
      console.error('保存用户登录状态失败:', error);
    }

    isLoggedIn = true;
    showAuth = false;

    // 用户登录成功后，开始初始化应用
    try {
      // 初始化数据库
      await EmailAPI.initDatabase();

      // 加载账户列表
      const accountList = await EmailAPI.getAccounts();
      accounts.set(accountList);

      // 如果有账户，选择第一个并为所有账户进行初始加载
      if (accountList.length > 0) {
        currentAccount.set(accountList[0]);

        // 为所有账户进行初始邮件加载，确保每个账户都有完整的邮件数据
        console.log('开始为所有账户进行初始邮件加载...');
        addNotification('info', '正在初始化所有邮件账户...');

        let successCount = 0;
        for (const account of accountList) {
          try {
            console.log(`正在加载账户: ${account.name}`);
            // 为每个账户加载邮件（优先使用缓存，缓存为空时才从服务器获取）
            await EmailAPI.getMessages(account.id, 'INBOX', 200, false);
            successCount++;
            console.log(`账户 ${account.name} 初始化成功`);
          } catch (err) {
            console.warn(`账户 ${account.name} 初始化失败:`, err);
          }
        }

        addNotification('success', `成功初始化 ${successCount}/${accountList.length} 个账户`);
        console.log(`账户初始化完成: ${successCount}/${accountList.length}`);
      }

      appInitialized = true;
      addNotification('success', `欢迎回来，${username}！应用初始化完成`);
    } catch (err) {
      console.error('初始化失败:', err);
      addNotification('error', `初始化失败: ${err}`);
      appInitialized = true; // 即使失败也要显示主界面
    }
  }


</script>

{#if showSplash}
  <SplashScreen onComplete={handleSplashComplete} />
{:else if showAuth}
  <AuthScreen on:loginSuccess={handleLoginSuccess} />
{:else}
  <div class="app">
    <InnovativeEmailInterface />

    <AddAccountDialog />
    <Notifications />
  </div>
{/if}

<style>
  :global(html, body) {
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', sans-serif;
    margin: 0;
    padding: 0;
    height: 100%;
    width: 100%;
    overflow: hidden;
  }

  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
    width: 100vw;
    background: #ffffff;
    overflow: hidden;
  }
</style>
