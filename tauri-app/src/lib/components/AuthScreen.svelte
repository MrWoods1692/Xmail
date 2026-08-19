<script lang="ts">
  import { createEventDispatcher, onMount } from 'svelte';
  import { fade } from 'svelte/transition';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';

  const dispatch = createEventDispatcher();
  
  let isLogin = true;
  let isLoading = false;
  
  // 登录表单数据
  let loginForm = {
    username: '',
    password: ''
  };
  
  // 注册表单数据
  let registerForm = {
    username: '',
    email: '',
    password: '',
    confirmPassword: ''
  };
  
  let errorMessage = '';
  
  // 切换登录/注册模式
  function toggleMode() {
    isLogin = !isLogin;
    errorMessage = '';
    // 清空表单
    loginForm = { username: '', password: '' };
    registerForm = { username: '', email: '', password: '', confirmPassword: '' };
  }
  
  // 处理登录
  async function handleLogin() {
    if (!loginForm.username || !loginForm.password) {
      errorMessage = '请填写用户名和密码';
      return;
    }

    isLoading = true;
    errorMessage = '';

    try {
      const response = await invoke('login_user', {
        loginData: {
          username: loginForm.username,
          password: loginForm.password
        }
      }) as { success: boolean; message: string; user?: any };

      if (response.success) {
        // 登录成功，触发事件
        dispatch('loginSuccess', {
          username: response.user?.username || loginForm.username,
          email: response.user?.email,
          authType: 'password',
          user: response.user
        });
      } else {
        errorMessage = response.message;
      }
    } catch (error) {
      console.error('登录错误:', error);
      errorMessage = '登录失败，请稍后重试';
    } finally {
      isLoading = false;
    }
  }
  
  // 处理注册
  async function handleRegister() {
    if (!registerForm.username || !registerForm.email || !registerForm.password) {
      errorMessage = '请填写所有必填字段';
      return;
    }

    if (registerForm.password !== registerForm.confirmPassword) {
      errorMessage = '两次输入的密码不一致';
      return;
    }

    if (registerForm.password.length < 6) {
      errorMessage = '密码长度至少6位';
      return;
    }

    isLoading = true;
    errorMessage = '';

    try {
      await invoke('register_user', {
        userData: {
          username: registerForm.username,
          email: registerForm.email,
          password: registerForm.password
        }
      });

      // 注册成功，自动切换到登录模式
      isLogin = true;
      errorMessage = '';
      loginForm.username = registerForm.username;
      registerForm = { username: '', email: '', password: '', confirmPassword: '' };

      // 显示成功消息
      setTimeout(() => {
        errorMessage = '注册成功！请登录';
      }, 100);
    } catch (error) {
      console.error('注册错误:', error);
      errorMessage = typeof error === 'string' ? error : '注册失败，请稍后重试';
    } finally {
      isLoading = false;
    }
  }

  // 处理Google OAuth2登录
  async function handleGoogleLogin() {
    if (isLoading) return;

    isLoading = true;
    errorMessage = '';

    try {
      console.log('启动Google OAuth2授权...');

      // 启动OAuth2授权窗口
      const oauthResult = await invoke('start_oauth2_window') as any;

      if (oauthResult && oauthResult.access_token) {
        console.log('Google OAuth2授权成功');

        // 获取用户信息（可选，用于显示用户名）
        try {
          const userInfo = await fetch('https://www.googleapis.com/oauth2/v2/userinfo', {
            headers: {
              'Authorization': `Bearer ${oauthResult.access_token}`
            }
          });

          if (userInfo.ok) {
            const userData = await userInfo.json();

            // 调用后端OAuth2登录命令（自动注册用户）
            try {
              console.log('调用oauth2_login_user命令，用户数据:', userData);

              // 为后端准备完整的用户信息，包括头像
              const userInfoForBackend = {
                ...userData,
                avatar: userData.picture, // Google API返回的头像字段是picture
                nickname: userData.name   // Google API返回的名称字段是name
              };

              const loginResponse = await invoke('oauth2_login_user', {
                userInfo: userInfoForBackend
              }) as { success: boolean; message: string; user?: any };
              console.log('oauth2_login_user响应:', loginResponse);

              if (loginResponse.success) {
                // OAuth2登录成功，触发登录成功事件
                dispatch('loginSuccess', {
                  username: userData.name || userData.email || 'Google用户',
                  email: userData.email,
                  authType: 'oauth2',
                  oauthToken: oauthResult,
                  user: loginResponse.user
                });
              } else {
                errorMessage = loginResponse.message;
              }
            } catch (dbError) {
              console.error('保存OAuth2用户信息失败:', dbError);
              // 即使保存失败，也允许登录
              dispatch('loginSuccess', {
                username: userData.name || userData.email || 'Google用户',
                email: userData.email,
                authType: 'oauth2',
                oauthToken: oauthResult
              });
            }
          } else {
            // 即使获取用户信息失败，OAuth2授权成功也算登录成功
            dispatch('loginSuccess', {
              username: 'Google用户',
              email: 'unknown@gmail.com',
              authType: 'oauth2',
              oauthToken: oauthResult
            });
          }
        } catch (userInfoError) {
          console.warn('获取Google用户信息失败，但OAuth2授权成功:', userInfoError);
          // 继续登录流程（无法保存到数据库，但允许登录）
          dispatch('loginSuccess', {
            username: 'Google用户',
            email: 'unknown@gmail.com',
            authType: 'oauth2',
            oauthToken: oauthResult
          });
        }
      } else {
        errorMessage = 'Google授权失败，请重试';
      }
    } catch (error) {
      console.error('Google OAuth2登录失败:', error);
      errorMessage = typeof error === 'string' ? error : 'Google登录失败，请稍后重试';
    } finally {
      isLoading = false;
    }
  }

  // 处理QQ OAuth2登录
  async function handleQQLogin() {
    if (isLoading) return;

    isLoading = true;
    errorMessage = '';

    try {
      console.log('启动QQ OAuth2授权...');

      // 调用Tauri命令启动QQ登录
      const result = await invoke('start_qq_oauth2_login') as any;

      if (result && result.success) {
        console.log('QQ登录成功:', result);

        // 触发登录成功事件
        dispatch('loginSuccess', {
          username: result.userInfo.nickname || 'QQ用户',
          email: 'QQ登录', // 显示友好的标识而不是复杂的openid
          authType: 'qq_oauth2',
          qqInfo: result.userInfo,
          user: result.user
        });
      } else {
        // 如果返回false，说明需要等待用户在浏览器中完成授权
        console.log('等待用户在浏览器中完成QQ授权...');
        errorMessage = result?.message || '请在授权窗口中完成QQ登录';

        // 后端会自动轮询检查授权状态，这里不需要前端轮询
      }
    } catch (error) {
      console.error('QQ OAuth2登录失败:', error);
      errorMessage = typeof error === 'string' ? error : 'QQ登录失败，请稍后重试';
      isLoading = false;
    }
  }





  // 主题配置
  const themes = {
    default: {
      primary: '#6c7ae0',
      secondary: '#7b68ee',
      hover: '#5a68d4',
      hoverSecondary: '#6c5ce7'
    },
    sunset: {
      primary: '#d4a574',
      secondary: '#e6c2a6',
      hover: '#b8956a',
      hoverSecondary: '#d4a574'
    },
    warm: {
      primary: '#fb7185',
      secondary: '#fda4af',
      hover: '#f43f5e',
      hoverSecondary: '#fb7185'
    },
    rose: {
      primary: '#f472b6',
      secondary: '#f9a8d4',
      hover: '#ec4899',
      hoverSecondary: '#f472b6'
    },
    coral: {
      primary: '#8b5cf6',
      secondary: '#a78bfa',
      hover: '#7c3aed',
      hoverSecondary: '#8b5cf6'
    }
  };

  // 更新主题颜色
  function updateThemeColors(themeName: keyof typeof themes) {
    const theme = themes[themeName];
    if (!theme) return;

    const root = document.documentElement;
    root.style.setProperty('--theme-primary', theme.primary);
    root.style.setProperty('--theme-secondary', theme.secondary);
    root.style.setProperty('--theme-hover', theme.hover);
    root.style.setProperty('--theme-hover-secondary', theme.hoverSecondary);
  }

  // 组件挂载时初始化主题和事件监听
  onMount(() => {
    try {
      const savedTheme = localStorage.getItem('selectedTheme') as keyof typeof themes;
      if (savedTheme && themes[savedTheme]) {
        updateThemeColors(savedTheme);
      } else {
        updateThemeColors('default');
      }
    } catch (error) {
      console.warn('加载主题设置失败:', error);
      updateThemeColors('default');
    }

    // 监听QQ登录成功事件
    const unlistenSuccess = listen('qq-login-success', async (event: any) => {
      console.log('收到QQ登录成功事件:', event.payload);
      const qqUserInfo = event.payload;

      try {
        // 调用OAuth2登录命令，自动注册/登录用户
        const loginResult = await invoke('oauth2_login_user', {
          userInfo: {
            openid: qqUserInfo.openid,
            nickname: qqUserInfo.nickname,
            email: `qq_${qqUserInfo.openid}@xmail.local`, // 使用更清晰的内部邮箱格式
            avatar: qqUserInfo.avatar,
            auth_type: 'qq_oauth2',
            provider: 'qq'
          }
        }) as any;

        if (loginResult.success) {
          // 触发登录成功事件
          dispatch('loginSuccess', {
            username: qqUserInfo.nickname || 'QQ用户',
            email: 'QQ登录', // 显示友好的标识而不是复杂的openid
            authType: 'qq_oauth2',
            qqInfo: qqUserInfo,
            user: loginResult.user
          });

          // 重置加载状态
          isLoading = false;
          errorMessage = '';
        } else {
          throw new Error(loginResult.message || 'QQ登录失败');
        }
      } catch (error) {
        console.error('QQ OAuth2登录处理失败:', error);
        errorMessage = typeof error === 'string' ? error : 'QQ登录失败，请稍后重试';
        isLoading = false;
      }
    });

    // 监听QQ登录错误事件
    const unlistenError = listen('qq-login-error', (event: any) => {
      console.log('收到QQ登录错误事件:', event.payload);
      errorMessage = typeof event.payload === 'string' ? event.payload : 'QQ登录失败';
      isLoading = false;
    });

    // 返回清理函数
    return () => {
      unlistenSuccess.then(fn => fn());
      unlistenError.then(fn => fn());
    };
  });


</script>

<div class="auth-container">
  <!-- 背景装饰 -->
  <div class="background-decoration">
    <div class="bubble bubble-1"></div>
    <div class="bubble bubble-2"></div>
    <div class="bubble bubble-3"></div>
    <div class="bubble bubble-4"></div>
  </div>
  
  <!-- 主要内容 -->
  <div class="auth-content">
    <!-- 左右布局容器 -->
    <div class="auth-layout">
      <!-- 登录模式：左边表单，右边图标 -->
      {#if isLogin}
        <!-- 左侧表单区域 -->
        <div class="form-section">
          <div class="form-header">
            <h1>欢迎回来</h1>
            <p class="form-subtitle">登录您的账户</p>
          </div>

          <!-- 模式切换按钮 -->
          <div class="mode-switcher">
            <button
              class="mode-btn {isLogin ? 'active' : ''}"
              on:click={() => !isLogin && toggleMode()}
            >
              登录
            </button>
            <button
              class="mode-btn {!isLogin ? 'active' : ''}"
              on:click={() => isLogin && toggleMode()}
            >
              注册
            </button>
            <div class="mode-indicator" class:register-mode={!isLogin}></div>
          </div>

          <!-- 错误消息 -->
          {#if errorMessage}
            <div class="error-message" transition:fade>
              {errorMessage}
            </div>
          {/if}

          <!-- 表单区域 -->
          <div class="form-area">
            <!-- 登录表单 -->
            <form class="auth-form" on:submit|preventDefault={handleLogin}>
              <div class="form-group">
                <label for="login-username">用户名</label>
                <input
                  id="login-username"
                  type="text"
                  bind:value={loginForm.username}
                  placeholder="请输入用户名"
                  disabled={isLoading}
                  required
                />
              </div>

              <div class="form-group">
                <label for="login-password">密码</label>
                <input
                  id="login-password"
                  type="password"
                  bind:value={loginForm.password}
                  placeholder="请输入密码"
                  disabled={isLoading}
                  required
                />
              </div>

              <button type="submit" class="submit-btn" disabled={isLoading}>
                {#if isLoading}
                  <div class="loading-spinner"></div>
                  登录中...
                {:else}
                  登录
                {/if}
              </button>
            </form>

            <!-- 快捷登录 -->
            <div class="quick-login-area">
              <div class="divider">
                <span>快捷登录</span>
              </div>
              <div class="quick-login-buttons">
                <button class="quick-login-btn" on:click={handleGoogleLogin} disabled={isLoading}>
                  {#if isLoading}
                    <div class="loading-spinner"></div>
                    授权中...
                  {:else}
                    <svg width="16" height="16" viewBox="0 0 24 24">
                      <path fill="#4285F4" d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z"/>
                      <path fill="#34A853" d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z"/>
                      <path fill="#FBBC05" d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z"/>
                      <path fill="#EA4335" d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z"/>
                    </svg>
                    Google
                  {/if}
                </button>
                <button class="quick-login-btn" on:click={handleQQLogin} disabled={isLoading}>
                  {#if isLoading}
                    <div class="loading-spinner"></div>
                    授权中...
                  {:else}
                    <img src="/QQkuaijiedenlu.png" alt="QQ" width="16" height="16" />
                    QQ
                  {/if}
                </button>
              </div>
            </div>
          </div>
        </div>

        <!-- 右侧图标区域 -->
          <div class="icon-section">
            <div class="icon-container">
              <img src="/denglu.svg" alt="登录" class="auth-icon" />
              <h2>XMail Pro</h2>
              <p>专业的邮件管理工具</p>
              <p class="description">安全、高效、智能的邮件体验</p>
            </div>
          </div>

      {:else}
        <!-- 注册模式：左边图标，右边表单 -->
        <!-- 左侧图标区域 -->
        <div class="icon-section">
          <div class="icon-container">
            <img src="/zuce.svg" alt="注册" class="auth-icon" />
            <h2>加入我们</h2>
            <p>开始您的邮件管理之旅</p>
            <p class="description">创建账户，体验全新的邮件管理方式</p>
          </div>
        </div>

        <!-- 右侧表单区域 -->
        <div class="form-section">
          <div class="form-header">
            <h1>创建账户</h1>
            <p class="form-subtitle">注册新用户</p>
          </div>

          <!-- 模式切换按钮 -->
          <div class="mode-switcher">
            <button
              class="mode-btn {isLogin ? 'active' : ''}"
              on:click={() => !isLogin && toggleMode()}
            >
              登录
            </button>
            <button
              class="mode-btn {!isLogin ? 'active' : ''}"
              on:click={() => isLogin && toggleMode()}
            >
              注册
            </button>
            <div class="mode-indicator" class:register-mode={!isLogin}></div>
          </div>

          <!-- 错误消息 -->
          {#if errorMessage}
            <div class="error-message" transition:fade>
              {errorMessage}
            </div>
          {/if}

          <!-- 表单区域 -->
          <div class="form-area">
            <!-- 注册表单 -->
            <form class="auth-form" on:submit|preventDefault={handleRegister}>
              <div class="form-group">
                <label for="register-username">用户名</label>
                <input
                  id="register-username"
                  type="text"
                  bind:value={registerForm.username}
                  placeholder="请输入用户名"
                  disabled={isLoading}
                  required
                />
              </div>

              <div class="form-group">
                <label for="register-email">邮箱</label>
                <input
                  id="register-email"
                  type="email"
                  bind:value={registerForm.email}
                  placeholder="请输入邮箱地址"
                  disabled={isLoading}
                  required
                />
              </div>

              <div class="form-group">
                <label for="register-password">密码</label>
                <input
                  id="register-password"
                  type="password"
                  bind:value={registerForm.password}
                  placeholder="请输入密码（至少6位）"
                  disabled={isLoading}
                  required
                />
              </div>

              <div class="form-group">
                <label for="register-confirm">确认密码</label>
                <input
                  id="register-confirm"
                  type="password"
                  bind:value={registerForm.confirmPassword}
                  placeholder="请再次输入密码"
                  disabled={isLoading}
                  required
                />
              </div>

              <button type="submit" class="submit-btn" disabled={isLoading}>
                {#if isLoading}
                  <div class="loading-spinner"></div>
                  注册中...
                {:else}
                  注册
                {/if}
              </button>
            </form>
          </div>
        </div>
      {/if}
    </div>
  </div>
</div>

<style>
  .auth-container {
    position: fixed;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100vh;
    background: linear-gradient(135deg, var(--theme-primary, #6c7ae0) 0%, var(--theme-secondary, #7b68ee) 100%);
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
  }

  .background-decoration {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
    overflow: hidden;
  }

  .bubble {
    position: absolute;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.1);
    backdrop-filter: blur(10px);
    animation: float 6s ease-in-out infinite;
  }

  .bubble-1 {
    width: 120px;
    height: 120px;
    top: 10%;
    left: 10%;
    animation-delay: 0s;
  }

  .bubble-2 {
    width: 80px;
    height: 80px;
    top: 20%;
    right: 15%;
    animation-delay: 2s;
  }

  .bubble-3 {
    width: 100px;
    height: 100px;
    bottom: 15%;
    left: 20%;
    animation-delay: 4s;
  }

  .bubble-4 {
    width: 60px;
    height: 60px;
    bottom: 25%;
    right: 10%;
    animation-delay: 1s;
  }

  @keyframes float {
    0%, 100% {
      transform: translateY(0px) rotate(0deg);
      opacity: 0.7;
    }
    50% {
      transform: translateY(-20px) rotate(180deg);
      opacity: 1;
    }
  }

  .auth-content {
    background: rgba(255, 255, 255, 0.95);
    backdrop-filter: blur(20px);
    border-radius: 24px;
    padding: 32px;
    box-shadow: 0 20px 40px rgba(0, 0, 0, 0.1);
    border: 1px solid rgba(255, 255, 255, 0.2);
    max-width: 900px;
    width: 90%;
    height: 600px;
    position: relative;
    z-index: 1;
    display: flex;
    flex-direction: column;
  }

  .auth-layout {
    display: flex;
    flex: 1;
    gap: 30px;
    align-items: center;
    justify-content: space-between;
    padding: 0 40px;
  }

  .form-section {
    flex: 0 0 auto;
    display: flex;
    flex-direction: column;
    width: 380px;
  }

  .icon-section {
    flex: 0 0 auto;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 300px;
  }

  .icon-container {
    text-align: center;
  }

  .auth-icon {
    width: 200px;
    height: 200px;
    margin-bottom: 24px;
  }

  .icon-container h2 {
    font-size: 28px;
    font-weight: 600;
    color: #1f2937;
    margin-bottom: 12px;
  }

  .icon-container p {
    font-size: 16px;
    color: #6b7280;
    margin-bottom: 8px;
  }

  .icon-container .description {
    font-size: 14px;
    color: #9ca3af;
  }

  .form-header {
    text-align: center;
    margin-bottom: 24px;
  }

  .form-header h1 {
    font-size: 24px;
    font-weight: 600;
    color: #1f2937;
    margin-bottom: 8px;
  }

  .form-subtitle {
    font-size: 14px;
    color: #6b7280;
  }



  .mode-switcher {
    position: relative;
    display: flex;
    background: #f3f4f6;
    border-radius: 12px;
    padding: 4px;
    margin-bottom: 24px;
  }

  .mode-btn {
    flex: 1;
    padding: 12px 16px;
    border: none;
    background: transparent;
    border-radius: 8px;
    font-size: 14px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.3s ease;
    position: relative;
    z-index: 2;
    color: #6b7280;
  }

  .mode-btn.active {
    color: var(--theme-primary, #6c7ae0);
  }

  .mode-indicator {
    position: absolute;
    top: 4px;
    left: 4px;
    width: calc(50% - 4px);
    height: calc(100% - 8px);
    background: white;
    border-radius: 8px;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
    transition: transform 0.3s ease;
    z-index: 1;
  }

  .mode-indicator.register-mode {
    transform: translateX(100%);
  }

  .error-message {
    background: #fef2f2;
    border: 1px solid #fecaca;
    color: #dc2626;
    padding: 12px 16px;
    border-radius: 8px;
    font-size: 14px;
    margin-bottom: 16px;
    text-align: center;
  }

  .form-area {
    flex: 1;
    display: flex;
    flex-direction: column;
    justify-content: flex-start;
  }

  .auth-form {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .form-group {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .form-group label {
    font-size: 14px;
    font-weight: 600;
    color: #374151;
  }

  .form-group input {
    padding: 10px 14px;
    border: 2px solid #e5e7eb;
    border-radius: 8px;
    font-size: 14px;
    transition: all 0.3s ease;
    background: white;
  }

  .form-group input:focus {
    outline: none;
    border-color: var(--theme-primary, #6c7ae0);
    box-shadow: 0 0 0 3px rgba(108, 122, 224, 0.1);
  }

  .form-group input:disabled {
    background: #f9fafb;
    cursor: not-allowed;
  }

  .submit-btn {
    padding: 14px 24px;
    background: linear-gradient(135deg, var(--theme-primary, #6c7ae0), var(--theme-secondary, #7b68ee));
    color: white;
    border: none;
    border-radius: 8px;
    font-size: 16px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.3s ease;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    margin-top: 8px;
  }

  .submit-btn:hover:not(:disabled) {
    transform: translateY(-1px);
    box-shadow: 0 4px 12px rgba(108, 122, 224, 0.3);
    background: linear-gradient(135deg, var(--theme-hover, #5a68d4), var(--theme-hover-secondary, #6c5ce7));
  }

  .submit-btn:disabled {
    opacity: 0.7;
    cursor: not-allowed;
    transform: none;
  }

  .loading-spinner {
    width: 16px;
    height: 16px;
    border: 2px solid rgba(255, 255, 255, 0.3);
    border-top: 2px solid white;
    border-radius: 50%;
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    0% { transform: rotate(0deg); }
    100% { transform: rotate(360deg); }
  }

  /* 快捷登录区域 */
  .quick-login-area {
    margin-top: 20px;
    padding-top: 20px;
  }

  .divider {
    position: relative;
    text-align: center;
    margin-bottom: 20px;
  }

  .divider::before {
    content: '';
    position: absolute;
    top: 50%;
    left: 0;
    right: 0;
    height: 2px;
    background: linear-gradient(90deg, transparent, rgba(0, 0, 0, 0.2), transparent);
  }

  .divider span {
    background: rgba(255, 255, 255, 0.95);
    backdrop-filter: blur(10px);
    padding: 8px 20px;
    color: #374151;
    font-size: 14px;
    font-weight: 600;
    border-radius: 20px;
    border: 1px solid rgba(255, 255, 255, 0.4);
    box-shadow: 0 2px 10px rgba(0, 0, 0, 0.1);
  }

  .quick-login-buttons {
    display: flex;
    flex-direction: row;
    gap: 12px;
    justify-content: center;
  }

  .quick-login-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 12px 20px;
    border: 1px solid rgba(255, 255, 255, 0.3);
    border-radius: 12px;
    background: rgba(255, 255, 255, 0.95);
    backdrop-filter: blur(20px);
    color: #374151;
    font-size: 14px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.3s ease;
    opacity: 1;
    min-width: 120px;
    box-shadow: 0 4px 15px rgba(0, 0, 0, 0.1);
  }

  .quick-login-btn:disabled {
    cursor: not-allowed;
    opacity: 0.6;
  }

  .quick-login-btn:not(:disabled):hover {
    background: rgba(255, 255, 255, 1);
    border-color: rgba(255, 255, 255, 0.5);
    transform: translateY(-2px);
    box-shadow: 0 8px 25px rgba(0, 0, 0, 0.15);
  }

  .quick-login-btn:not(:disabled):active {
    transform: translateY(0);
    box-shadow: 0 4px 15px rgba(0, 0, 0, 0.1);
  }




</style>
