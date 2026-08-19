<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { showAccountDialog, accounts, currentAccount, messages, addNotification, preselectedProvider, currentUser } from '../store';
  import { EmailAPI } from '../api';
  import type { EmailMessage, EmailTag, EmailProviderConfig, EmailAccount, Contact } from '../types';
  import { listen } from '@tauri-apps/api/event';
  import { convertFileSrc, invoke } from '@tauri-apps/api/core';
  import { getSenderDisplayName, getSenderEmail, getSenderAvatarUrl, clearAvatarCache } from '../utils/avatarUtils';
  import ComposeEmailDialog from './ComposeEmailDialog.svelte';
  import LoadingProgress from './LoadingProgress.svelte';
  import AddContactDialog from './AddContactDialog.svelte';

  let viewMode: 'list' | 'timeline' | 'chat' = 'list';
  let searchQuery = '';
  let selectedEmail: EmailMessage | null = null;
  let currentFolder: 'inbox' | 'sent' | 'drafts' | 'spam' | 'trash' | 'starred' = 'inbox';
  let showComposeDialog = false;
  let isLoadingMessages = false;
  let loadingStep = 0;
  let totalLoadingSteps = 4;
  let isWindowMaximized = false;

  // 设置页面状态
  let showSettingsPage = false;
  let currentSettingsTab = 'personal'; // 'personal' | 'email' | 'mail-notifications' | 'theme' | 'system'

  // 侧边栏切换状态
  let sidebarMode: 'accounts' | 'contacts' = 'accounts'; // 'accounts' | 'contacts'

  // 联系人相关状态
  let contacts: Contact[] = [];
  let showAddContactDialog = false;
  let isLoadingContacts = false;

  // 系统设置状态
  let customDatabasePath = '';
  let currentDatabasePath = '';
  let showDatabasePathDialog = false;
  let migrationOption = 'migrate'; // 'migrate' | 'fresh'
  let titleBarStyle = 'windows'; // 'windows' | 'mac'
  let titleBarStyleLoaded = false; // 标记是否已加载设置

  // 加载标题栏样式设置
  onMount(async () => {
    try {
      const savedStyle = localStorage.getItem('titleBarStyle');
      if (savedStyle && (savedStyle === 'windows' || savedStyle === 'mac')) {
        titleBarStyle = savedStyle;
      }
      titleBarStyleLoaded = true; // 标记已加载
    } catch (error) {
      console.warn('加载标题栏样式设置失败:', error);
      titleBarStyleLoaded = true;
    }

    // 加载邮件预览动画设置
    try {
      const savedAnimation = localStorage.getItem('emailPreviewAnimation');
      if (savedAnimation && (savedAnimation === 'slideUp' || savedAnimation === 'slideRight' || savedAnimation === 'slideLeft')) {
        emailPreviewAnimation = savedAnimation;
      }
      emailPreviewAnimationLoaded = true; // 标记已加载
    } catch (error) {
      console.warn('加载邮件预览动画设置失败:', error);
      emailPreviewAnimationLoaded = true;
    }
  });

  // 监听标题栏样式变化并保存（只在加载完成后保存）
  $: if (titleBarStyle && titleBarStyleLoaded) {
    try {
      localStorage.setItem('titleBarStyle', titleBarStyle);
      console.log('标题栏样式已保存:', titleBarStyle);
    } catch (error) {
      console.warn('保存标题栏样式设置失败:', error);
    }
  }

  // 监听邮件预览动画变化并保存（只在加载完成后保存）
  $: if (emailPreviewAnimation && emailPreviewAnimationLoaded) {
    try {
      localStorage.setItem('emailPreviewAnimation', emailPreviewAnimation);
      console.log('邮件预览动画已保存:', emailPreviewAnimation);
    } catch (error) {
      console.warn('保存邮件预览动画设置失败:', error);
    }
  }

  // 通知设置状态
  let notificationSettings = {
    enableSound: true,
    selectedSound: 'lingsheng1', // 'lingsheng1' | 'lingsheng2'
    enableDesktopNotification: true
  };

  // 邮件预览动画设置
  let emailPreviewAnimation = 'slideUp'; // 'slideUp' | 'slideRight' | 'slideLeft'
  let emailPreviewAnimationLoaded = false; // 标记是否已加载设置

  // 主题设置
  let currentTheme: keyof typeof themes | 'custom' = 'default';

  // 自定义主题设置
  let customTheme = {
    name: '自定义主题',
    primary: '#6c7ae0',
    secondary: '#7b68ee',
    hover: '#5a68d4',
    hoverSecondary: '#6c5ce7'
  };

  // 自定义主题对话框状态
  let showCustomThemeDialog = false;

  // 翻译功能状态
  let isTranslating = false;
  let translatedEmails: Record<string, any> = {}; // 存储翻译后的邮件内容
  let showOriginal: Record<string, boolean> = {}; // 控制是否显示原始内容

  // 主题配置
  const themes = {
    default: {
      name: '默认紫蓝',
      primary: '#6c7ae0',
      secondary: '#7b68ee',
      hover: '#5a68d4',
      hoverSecondary: '#6c5ce7'
    },
    sunset: {
      name: '奶茶色',
      primary: '#d4a574',
      secondary: '#e6c2a6',
      hover: '#b8956a',
      hoverSecondary: '#d4a574'
    },
    warm: {
      name: '珊瑚橙',
      primary: '#fb7185',
      secondary: '#fda4af',
      hover: '#f43f5e',
      hoverSecondary: '#fb7185'
    },
    rose: {
      name: '玫瑰金',
      primary: '#f472b6',
      secondary: '#f9a8d4',
      hover: '#ec4899',
      hoverSecondary: '#f472b6'
    },
    coral: {
      name: '薰衣草',
      primary: '#8b5cf6',
      secondary: '#a78bfa',
      hover: '#7c3aed',
      hoverSecondary: '#8b5cf6'
    }
  };

  // 主题切换函数
  function switchTheme(themeName: keyof typeof themes | 'custom') {
    currentTheme = themeName;
    localStorage.setItem('selectedTheme', themeName);
    updateThemeColors();
  }

  // 十六进制颜色转RGB
  function hexToRgb(hex: string) {
    const result = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i.exec(hex);
    return result ? {
      r: parseInt(result[1], 16),
      g: parseInt(result[2], 16),
      b: parseInt(result[3], 16)
    } : null;
  }

  // 更新主题颜色
  function updateThemeColors() {
    let theme;
    if (currentTheme === 'custom') {
      theme = customTheme;
    } else {
      theme = themes[currentTheme as keyof typeof themes];
    }

    if (!theme) return;

    const root = document.documentElement;
    root.style.setProperty('--theme-primary', theme.primary);
    root.style.setProperty('--theme-secondary', theme.secondary);
    root.style.setProperty('--theme-hover', theme.hover);
    root.style.setProperty('--theme-hover-secondary', theme.hoverSecondary);

    // 设置RGB版本的主题色
    const primaryRgb = hexToRgb(theme.primary);
    if (primaryRgb) {
      root.style.setProperty('--theme-primary-rgb', `${primaryRgb.r}, ${primaryRgb.g}, ${primaryRgb.b}`);
    }
  }

  // 打开自定义主题对话框
  function openCustomThemeDialog() {
    showCustomThemeDialog = true;
  }

  // 关闭自定义主题对话框
  function closeCustomThemeDialog() {
    showCustomThemeDialog = false;
  }

  // 应用自定义主题
  function applyCustomTheme() {
    // 最终验证所有颜色（应用时显示通知）
    const originalPrimary = customTheme.primary;
    const originalSecondary = customTheme.secondary;

    customTheme.primary = validateAndFixColor(customTheme.primary, '#6c7ae0', true);
    customTheme.secondary = validateAndFixColor(customTheme.secondary, '#7b68ee', true);
    customTheme.hover = validateAndFixColor(customTheme.hover, '#5a68d4', false);
    customTheme.hoverSecondary = validateAndFixColor(customTheme.hoverSecondary, '#6c5ce7', false);

    // 保存自定义主题到localStorage
    localStorage.setItem('customTheme', JSON.stringify(customTheme));

    // 切换到自定义主题
    switchTheme('custom');

    // 关闭对话框
    closeCustomThemeDialog();

    addNotification('success', '自定义主题已应用');
  }

  // 重置自定义主题为默认值
  function resetCustomTheme() {
    customTheme = {
      name: '自定义主题',
      primary: '#6c7ae0',
      secondary: '#7b68ee',
      hover: '#5a68d4',
      hoverSecondary: '#6c5ce7'
    };
  }

  // 从颜色生成悬停色
  function generateHoverColor(color: string): string {
    try {
      // 验证颜色格式
      if (!color || typeof color !== 'string') {
        return '#5a68d4'; // 默认悬停色
      }

      // 确保颜色以#开头且长度正确
      const hex = color.replace('#', '');
      if (hex.length !== 6) {
        return '#5a68d4'; // 默认悬停色
      }

      // 验证是否为有效的十六进制颜色
      if (!/^[0-9A-Fa-f]{6}$/.test(hex)) {
        return '#5a68d4'; // 默认悬停色
      }

      const r = parseInt(hex.substring(0, 2), 16);
      const g = parseInt(hex.substring(2, 4), 16);
      const b = parseInt(hex.substring(4, 6), 16);

      // 验证RGB值是否有效
      if (isNaN(r) || isNaN(g) || isNaN(b)) {
        return '#5a68d4'; // 默认悬停色
      }

      const darkenFactor = 0.8;
      const newR = Math.floor(r * darkenFactor);
      const newG = Math.floor(g * darkenFactor);
      const newB = Math.floor(b * darkenFactor);

      return `#${newR.toString(16).padStart(2, '0')}${newG.toString(16).padStart(2, '0')}${newB.toString(16).padStart(2, '0')}`;
    } catch (error) {
      console.error('生成悬停色时出错:', error);
      return '#5a68d4'; // 默认悬停色
    }
  }

  // 检查颜色是否过于接近白色
  function isColorTooLight(color: string): boolean {
    try {
      const hex = color.replace('#', '');
      if (hex.length !== 6) return false;

      const r = parseInt(hex.substring(0, 2), 16);
      const g = parseInt(hex.substring(2, 4), 16);
      const b = parseInt(hex.substring(4, 6), 16);

      // 计算亮度 (使用相对亮度公式)
      const luminance = (0.299 * r + 0.587 * g + 0.114 * b) / 255;

      // 如果亮度大于0.85，认为颜色过浅
      return luminance > 0.85;
    } catch (error) {
      return false;
    }
  }

  // 防抖通知状态
  let lastColorWarningTime = 0;
  const COLOR_WARNING_COOLDOWN = 3000; // 3秒冷却时间

  // 验证并修正颜色
  function validateAndFixColor(color: string, fallbackColor: string, showNotification: boolean = false): string {
    if (isColorTooLight(color)) {
      // 只在需要显示通知且距离上次通知超过冷却时间时才显示
      if (showNotification) {
        const now = Date.now();
        if (now - lastColorWarningTime > COLOR_WARNING_COOLDOWN) {
          addNotification('warning', '颜色过浅，已自动调整为较深的颜色以确保按钮可见性');
          lastColorWarningTime = now;
        }
      }
      return fallbackColor;
    }
    return color;
  }

  // 当主色调改变时自动更新悬停色
  function onPrimaryColorChange() {
    try {
      // 验证颜色是否过浅（实时输入时不显示通知）
      customTheme.primary = validateAndFixColor(customTheme.primary, '#6c7ae0', false);
      customTheme.hover = generateHoverColor(customTheme.primary);
    } catch (error) {
      console.error('更新主要悬停色时出错:', error);
    }
  }

  // 当次要色调改变时自动更新悬停色
  function onSecondaryColorChange() {
    try {
      // 验证颜色是否过浅（实时输入时不显示通知）
      customTheme.secondary = validateAndFixColor(customTheme.secondary, '#7b68ee', false);
      customTheme.hoverSecondary = generateHoverColor(customTheme.secondary);
    } catch (error) {
      console.error('更新次要悬停色时出错:', error);
    }
  }

  // 检测文本语言
  function detectLanguage(text: string): string {
    // 简单的语言检测：如果包含中文字符，认为是中文，否则认为是英文
    const chineseRegex = /[\u4e00-\u9fff]/;
    return chineseRegex.test(text) ? 'zh' : 'en';
  }

  // 翻译邮件内容（内联翻译）
  async function translateEmail(email: any) {
    // 检查是否已经翻译过
    if (translatedEmails[email.id]) {
      addNotification('info', '该邮件已翻译，切换显示模式');
      return;
    }

    // 检查是否正在翻译（使用邮件ID作为标识）
    if (isTranslating) {
      addNotification('warning', '请等待当前翻译完成');
      return;
    }

    try {
      isTranslating = true;
      addNotification('info', '正在翻译邮件内容...');

      // 检测语言
      let sourceLanguage = 'en';
      let targetLanguage = 'zh';

      if (email.body_html) {
        const tempDiv = document.createElement('div');
        tempDiv.innerHTML = email.body_html;
        const textContent = tempDiv.textContent || tempDiv.innerText || '';
        sourceLanguage = detectLanguage(textContent);
        targetLanguage = sourceLanguage === 'zh' ? 'en' : 'zh';
      } else if (email.body_text) {
        sourceLanguage = detectLanguage(email.body_text);
        targetLanguage = sourceLanguage === 'zh' ? 'en' : 'zh';
      }

      const languageNames: Record<string, string> = { zh: '中文', en: '英文' };
      console.log(`翻译邮件 ${email.id}: ${languageNames[sourceLanguage]} → ${languageNames[targetLanguage]}`);

      // 翻译邮件内容（简化高效方案）
      let translatedHtml = '';
      let translatedText = '';

      if (email.body_html) {
        console.log('开始翻译HTML内容...');
        translatedHtml = await translateHtmlContentFast(email.body_html, sourceLanguage, targetLanguage);
        console.log('HTML内容翻译完成');
      }

      if (email.body_text) {
        console.log('开始翻译纯文本内容...');

        // 为纯文本邮件也创建实时更新机制
        if (!translatedEmails[email.id]) {
          translatedEmails[email.id] = {
            ...email,
            body_html: translatedHtml || email.body_html,
            body_text: email.body_text, // 先保持原文本
            sourceLanguage,
            targetLanguage,
            isTranslated: true,
            isTranslating: true
          };
          translatedEmails = translatedEmails; // 触发响应式更新
        }

        translatedText = await callOllamaTranslate(email.body_text, sourceLanguage, targetLanguage);

        // 更新翻译后的文本
        if (translatedEmails[email.id]) {
          translatedEmails[email.id].body_text = translatedText;
          translatedEmails[email.id].isTranslating = false;
          translatedEmails = translatedEmails; // 触发响应式更新
        }

        console.log('纯文本内容翻译完成');
      }

      // 确保翻译对象存在并更新最终状态
      if (!translatedEmails[email.id]) {
        translatedEmails[email.id] = {
          ...email,
          body_html: translatedHtml || email.body_html,
          body_text: translatedText || email.body_text,
          sourceLanguage,
          targetLanguage,
          isTranslated: true,
          isTranslating: false
        };
      } else {
        // 更新已存在的翻译对象
        translatedEmails[email.id] = {
          ...translatedEmails[email.id],
          body_html: translatedHtml || translatedEmails[email.id].body_html,
          body_text: translatedText || translatedEmails[email.id].body_text,
          isTranslating: false
        };
      }

      // 触发响应式更新
      translatedEmails = translatedEmails;
      console.log('翻译结果已保存，邮件ID:', email.id);

      addNotification('success', `翻译完成：${languageNames[sourceLanguage] || sourceLanguage} → ${languageNames[targetLanguage] || targetLanguage}`);
    } catch (error) {
      console.error('翻译失败:', error);
      const errorMessage = error instanceof Error ? error.message : String(error);
      addNotification('error', `翻译失败: ${errorMessage}`);
    } finally {
      isTranslating = false;
      console.log('翻译状态已重置');
    }
  }

  // 翻译HTML内容，保持格式（优化版本）
  async function translateHtmlContent(htmlContent: string, sourceLanguage: string, targetLanguage: string): Promise<string> {
    try {
      const tempDiv = document.createElement('div');
      tempDiv.innerHTML = htmlContent;

      // 获取所有需要翻译的文本节点
      const textNodes = [];
      const walker = document.createTreeWalker(
        tempDiv,
        NodeFilter.SHOW_TEXT,
        {
          acceptNode: function(node) {
            // 跳过空白节点、样式节点和脚本节点
            if (!node.textContent?.trim() ||
                node.parentElement?.tagName === 'STYLE' ||
                node.parentElement?.tagName === 'SCRIPT') {
              return NodeFilter.FILTER_REJECT;
            }
            return NodeFilter.FILTER_ACCEPT;
          }
        }
      );

      let node;
      while (node = walker.nextNode()) {
        const text = node.textContent?.trim();
        if (text && text.length > 2) {
          textNodes.push({
            node: node,
            originalText: text
          });
        }
      }

      console.log(`找到 ${textNodes.length} 个文本节点需要翻译`);

      if (textNodes.length === 0) {
        console.warn('HTML内容中没有可翻译的文本');
        return htmlContent;
      }

      // 分块翻译策略
      const CHUNK_SIZE = 5; // 每次翻译5个文本节点
      const MAX_CONCURRENT = 3; // 最多同时进行3个翻译请求

      // 将文本节点分组
      const chunks = [];
      for (let i = 0; i < textNodes.length; i += CHUNK_SIZE) {
        chunks.push(textNodes.slice(i, i + CHUNK_SIZE));
      }

      console.log(`分成 ${chunks.length} 个块进行翻译`);

      // 并发翻译函数
      async function translateChunk(chunk: any[]) {
        const texts = chunk.map(item => item.originalText);
        const combinedText = texts.join('\n---SEP---\n');

        try {
          const translatedCombined = await callOllamaTranslate(combinedText, sourceLanguage, targetLanguage);
          const translatedTexts = translatedCombined.split('\n---SEP---\n');

          // 应用翻译结果
          for (let i = 0; i < chunk.length; i++) {
            if (translatedTexts[i]) {
              chunk[i].node.textContent = translatedTexts[i].trim();
            }
          }

          return true;
        } catch (error) {
          console.warn('块翻译失败，使用逐个翻译:', error);
          // 备用方案：逐个翻译
          for (const item of chunk) {
            try {
              const translated = await callOllamaTranslate(item.originalText, sourceLanguage, targetLanguage);
              item.node.textContent = translated;
            } catch (err) {
              console.warn('单个文本翻译失败:', err);
              // 保持原文本
            }
          }
          return false;
        }
      }

      // 控制并发数量的翻译
      const results = [];
      for (let i = 0; i < chunks.length; i += MAX_CONCURRENT) {
        const currentBatch = chunks.slice(i, i + MAX_CONCURRENT);
        const batchPromises = currentBatch.map(chunk => translateChunk(chunk));

        console.log(`正在翻译第 ${i + 1}-${Math.min(i + MAX_CONCURRENT, chunks.length)} 批次...`);
        const batchResults = await Promise.all(batchPromises);
        results.push(...batchResults);

        // 添加小延迟，避免API限流
        if (i + MAX_CONCURRENT < chunks.length) {
          await new Promise(resolve => setTimeout(resolve, 100));
        }
      }

      console.log('所有翻译完成');
      return tempDiv.innerHTML;

    } catch (error) {
      console.error('HTML内容翻译失败:', error);
      return htmlContent; // 如果翻译失败，返回原内容
    }
  }

  // 流式翻译HTML内容，实时显示翻译进度
  async function translateHtmlContentFast(htmlContent: string, sourceLanguage: string, targetLanguage: string): Promise<string> {
    try {
      const tempDiv = document.createElement('div');
      tempDiv.innerHTML = htmlContent;

      // 获取所有文本节点，并记录它们的原始文本
      const textNodes: { node: Node; originalText: string; index: number }[] = [];
      const walker = document.createTreeWalker(
        tempDiv,
        NodeFilter.SHOW_TEXT,
        {
          acceptNode: function(node) {
            const text = node.textContent?.trim();
            if (!text || text.length < 2 ||
                node.parentElement?.tagName === 'STYLE' ||
                node.parentElement?.tagName === 'SCRIPT') {
              return NodeFilter.FILTER_REJECT;
            }
            return NodeFilter.FILTER_ACCEPT;
          }
        }
      );

      let node;
      let index = 0;
      while (node = walker.nextNode()) {
        const text = node.textContent?.trim();
        if (text) {
          textNodes.push({
            node: node,
            originalText: text,
            index: index++
          });
        }
      }

      console.log(`找到 ${textNodes.length} 个文本节点需要翻译`);

      if (textNodes.length === 0) {
        return htmlContent;
      }

      // 创建临时翻译邮件对象，用于实时更新显示
      const emailId = selectedEmail?.id;
      if (emailId && selectedEmail) {
        // 先创建一个部分翻译的邮件对象
        translatedEmails[emailId] = {
          ...selectedEmail,
          body_html: tempDiv.innerHTML,
          body_text: selectedEmail.body_text,
          sourceLanguage,
          targetLanguage,
          isTranslated: true,
          isTranslating: true // 标记正在翻译中
        };
        translatedEmails = translatedEmails; // 触发响应式更新
      }

      // 分批翻译，每批10个节点
      const BATCH_SIZE = 10;
      for (let i = 0; i < textNodes.length; i += BATCH_SIZE) {
        const batch = textNodes.slice(i, i + BATCH_SIZE);

        console.log(`🔄 翻译第 ${Math.floor(i/BATCH_SIZE) + 1}/${Math.ceil(textNodes.length/BATCH_SIZE)} 批 (节点 ${i + 1}-${Math.min(i + BATCH_SIZE, textNodes.length)})`);

        // 并发翻译当前批次
        const batchPromises = batch.map(async (item) => {
          try {
            const translated = await callOllamaTranslateFast(item.originalText, sourceLanguage, targetLanguage);
            item.node.textContent = translated;
            console.log(`✅ 节点 ${item.index + 1} 翻译完成`);
            return true;
          } catch (error) {
            console.warn(`❌ 节点 ${item.index + 1} 翻译失败:`, error);
            return false;
          }
        });

        // 等待当前批次完成
        await Promise.all(batchPromises);

        // 实时更新显示
        if (emailId && translatedEmails[emailId]) {
          translatedEmails[emailId].body_html = tempDiv.innerHTML;
          translatedEmails = translatedEmails; // 触发响应式更新

          // 添加小延迟，让用户看到翻译进度
          await new Promise(resolve => setTimeout(resolve, 100));
        }
      }

      // 标记翻译完成
      if (emailId && translatedEmails[emailId]) {
        translatedEmails[emailId].isTranslating = false;
        translatedEmails = translatedEmails;
      }

      return tempDiv.innerHTML;

    } catch (error) {
      console.error('HTML翻译失败:', error);
      return htmlContent;
    }
  }

  // 快速翻译API调用（简化版）
  async function callOllamaTranslateFast(text: string, sourceLanguage: string, targetLanguage: string): Promise<string> {
    const ollamaUrl = 'http://106.14.123.54:11434';

    // 更精确的提示词，避免格式错乱
    let prompt = '';
    if (sourceLanguage === 'zh' && targetLanguage === 'en') {
      prompt = `Translate the following Chinese text to English. Keep the exact same format and structure. Do not add any explanations or extra content. Only output the direct translation:

${text}`;
    } else if (sourceLanguage === 'en' && targetLanguage === 'zh') {
      prompt = `将以下英文翻译成中文。保持完全相同的格式和结构。不要添加任何解释或额外内容。只输出直接翻译结果。

翻译原则：
- 对于广为人知的英文缩写（如API、URL、HTTP、FAQ、CEO、AI、ML等），保持原样
- 对于网络用语缩写（如TL;DR、IMHO、AFAIK等），可以适当本地化或保持原样
- 对于专业术语和品牌名称，保持英文原样
- 其他内容正常翻译成自然的中文

要翻译的文本：
${text}`;
    }

    const response = await fetch(`${ollamaUrl}/api/generate`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        model: 'qwen2.5:3b',
        prompt: prompt,
        stream: false,
        options: {
          temperature: 0,
          top_p: 0.7,
          num_predict: 1024
        }
      })
    });

    if (!response.ok) {
      throw new Error(`API请求失败: ${response.status}`);
    }

    const data = await response.json();

    if (!data.response || data.response.trim() === '') {
      throw new Error('翻译返回空结果');
    }

    // 清理翻译结果，移除可能的多余内容
    let result = data.response.trim();

    // 移除常见的AI回复前缀
    const prefixes = ['翻译结果：', '翻译：', 'Translation:', 'Result:', '结果：'];
    for (const prefix of prefixes) {
      if (result.startsWith(prefix)) {
        result = result.substring(prefix.length).trim();
      }
    }

    return result;
  }

  // 调用Ollama API进行翻译（保留原函数用于纯文本翻译）
  async function callOllamaTranslate(text: string, sourceLanguage: string, targetLanguage: string): Promise<string> {
    return await callOllamaTranslateFast(text, sourceLanguage, targetLanguage);
  }

  // 切换原始内容和翻译内容
  function toggleTranslation(emailId: string) {
    const currentState = showOriginal[emailId] || false;
    showOriginal[emailId] = !currentState;
    showOriginal = showOriginal; // 触发响应式更新
  }

  // 设置链接拦截，让邮件中的链接在外部浏览器打开
  function setupLinkInterception() {
    if (typeof window === 'undefined') return;

    // 监听所有点击事件
    document.addEventListener('click', async (event) => {
      const target = event.target as HTMLElement;

      // 检查是否点击了链接
      const link = target.closest('a[href]') as HTMLAnchorElement;
      if (!link) return;

      // 检查链接是否在邮件内容区域内
      const emailBody = link.closest('.email-body');
      if (!emailBody) return;

      // 阻止默认行为
      event.preventDefault();
      event.stopPropagation();

      const href = link.href;
      console.log('拦截邮件链接:', href);

      try {
        // 使用 Tauri v2 的 opener 插件在外部浏览器打开链接
        const { openUrl } = await import('@tauri-apps/plugin-opener');
        await openUrl(href);
        console.log('已在外部浏览器打开链接:', href);
      } catch (error) {
        console.error('打开外部链接失败:', error);
        addNotification('error', '无法打开链接');
      }
    }, true); // 使用捕获阶段，确保能拦截到事件

    console.log('链接拦截已设置');
  }

  // 获取要显示的邮件内容
  function getDisplayEmail(email: any) {
    if (translatedEmails[email.id] && !showOriginal[email.id]) {
      return translatedEmails[email.id];
    }
    return email;
  }

  // 打开设置页面
  function openSettingsPage() {
    showSettingsPage = true;
    selectedEmail = null; // 关闭邮件详情
    // 获取当前数据库路径
    getCurrentDatabasePath();
  }

  // 关闭设置页面
  function closeSettingsPage() {
    showSettingsPage = false;
  }

  // 退出登录
  function logout() {
    // 清除用户信息
    currentUser.set(null);

    // 清除保存的用户登录状态
    try {
      localStorage.removeItem('currentUser');
      console.log('已清除保存的用户登录状态');
    } catch (error) {
      console.error('清除用户登录状态失败:', error);
    }

    // 清除账户信息
    accounts.set([]);
    currentAccount.set(null);

    // 清除邮件信息
    messages.set([]);
    selectedEmail = null;

    // 关闭设置页面
    showSettingsPage = false;

    // 重新加载页面回到登录界面
    window.location.reload();

    addNotification('success', '已成功退出登录');
  }

  // 切换设置标签页
  function switchSettingsTab(tab: string) {
    currentSettingsTab = tab;
  }

  // 切换侧边栏模式
  function switchSidebarMode(mode: 'accounts' | 'contacts') {
    sidebarMode = mode;
    if (mode === 'contacts') {
      loadContacts();
    }
  }

  // 加载联系人列表
  async function loadContacts() {
    if (!$currentUser?.id) return;

    isLoadingContacts = true;
    try {
      contacts = await EmailAPI.getContacts($currentUser.id);
      console.log('联系人加载成功:', contacts.length);
    } catch (error) {
      console.error('加载联系人失败:', error);
      addNotification('error', '加载联系人失败');
    } finally {
      isLoadingContacts = false;
    }
  }

  // 打开添加联系人对话框
  function openAddContactDialog() {
    showAddContactDialog = true;
  }

  // 处理联系人添加成功
  function handleContactAdded() {
    loadContacts(); // 重新加载联系人列表
    addNotification('success', '联系人添加成功');
  }

  // 打开写邮件对话框并预填联系人邮箱
  function openComposeEmailToContact(contact: Contact) {
    showComposeDialog = true;
    // 使用自定义事件传递联系人信息到写邮件对话框
    setTimeout(() => {
      window.dispatchEvent(new CustomEvent('composeToContact', {
        detail: {
          to: contact.email,
          name: contact.name
        }
      }));
    }, 100);
  }

  // 删除邮箱账号
  async function deleteEmailAccount(account: EmailAccount) {
    if (!confirm(`确定要删除邮箱账号 "${account.name}" (${account.email}) 吗？\n\n此操作将删除该账号的所有邮件缓存，且无法撤销。`)) {
      return;
    }

    try {
      // 调用后端删除账号
      await EmailAPI.deleteAccount(account.id);

      // 更新本地账号列表
      accounts.update(accs => accs.filter(acc => acc.id !== account.id));

      // 如果删除的是当前选中的账号，清除选中状态
      if ($currentAccount?.id === account.id) {
        currentAccount.set(null);
        messages.set([]);
        selectedEmail = null;
      }

      addNotification('success', `邮箱账号 "${account.name}" 已删除`);
    } catch (error) {
      console.error('删除邮箱账号失败:', error);
      addNotification('error', `删除账号失败: ${error}`);
    }
  }

  // 获取当前数据库路径
  async function getCurrentDatabasePath() {
    try {
      currentDatabasePath = await invoke('get_database_path');
    } catch (error) {
      console.error('获取数据库路径失败:', error);
      addNotification('error', `获取数据库路径失败: ${error}`);
    }
  }

  // 选择数据库路径
  async function selectDatabasePath() {
    try {
      // 使用动态导入来避免TypeScript类型检查问题
      const dialogModule = await import('@tauri-apps/plugin-dialog');

      const filePath = await dialogModule.save({
        title: '选择数据库保存位置',
        defaultPath: 'emails.db',
        filters: [
          {
            name: 'SQLite数据库',
            extensions: ['db', 'sqlite', 'sqlite3']
          }
        ]
      });

      if (filePath) {
        customDatabasePath = filePath;
        addNotification('success', '数据库路径已选择');
      }
    } catch (error) {
      console.error('选择数据库路径失败:', error);
      addNotification('error', `选择数据库路径失败: ${error}`);
    }
  }

  // 应用数据库路径设置
  async function applyDatabasePath() {
    if (!customDatabasePath.trim()) {
      addNotification('error', '请选择数据库路径');
      return;
    }

    try {
      await invoke('set_database_path', {
        path: customDatabasePath,
        migrateData: migrationOption === 'migrate'
      });

      // 更新当前数据库路径显示
      currentDatabasePath = customDatabasePath;

      if (migrationOption === 'migrate') {
        addNotification('success', '数据库路径设置成功，数据已迁移，重启应用后生效');
      } else {
        addNotification('success', '数据库路径设置成功，将创建新数据库，重启应用后生效');
      }

      showDatabasePathDialog = false;
      await getCurrentDatabasePath();
    } catch (error) {
      console.error('设置数据库路径失败:', error);
      addNotification('error', `设置数据库路径失败: ${error}`);
    }
  }

  // 重置数据库路径到默认位置
  async function resetDatabasePath() {
    try {
      await invoke('reset_database_path');
      addNotification('success', '数据库路径已重置为默认位置，重启应用后生效');
      await getCurrentDatabasePath();
    } catch (error) {
      console.error('重置数据库路径失败:', error);
      addNotification('error', `重置数据库路径失败: ${error}`);
    }
  }

  // 播放通知铃声
  async function playNotificationSound(soundName?: string) {
    if (!notificationSettings.enableSound) return;

    const sound = soundName || notificationSettings.selectedSound;
    const startTime = Date.now();
    console.log(`🔊 [${new Date().toLocaleTimeString()}] 开始播放铃声:`, sound);

    // 检查浏览器是否支持音频
    if (typeof Audio === 'undefined') {
      console.error('❌ 浏览器不支持Audio API');
      return;
    }

    const testPaths = [
      // 生产环境静态资源路径（优先）
      `/assets/Notification/${sound}.wav`,
      // 开发环境路径
      `/src/assets/Notification/${sound}.wav`,
      `./src/assets/Notification/${sound}.wav`,
      // Tauri资源路径
      convertFileSrc(`${sound}.wav`, 'resource'),
      // 根目录路径
      `/${sound}.wav`,
      // 尝试不同的扩展名
      `/src/assets/Notification/${sound}.mp3`,
    ];

    for (let i = 0; i < testPaths.length; i++) {
      const path = testPaths[i];
      console.log(`🎵 尝试路径 ${i + 1}:`, path);

      try {
        const audio = new Audio(path);
        audio.volume = 0.5;

        // 等待音频加载
        await new Promise((resolve, reject) => {
          const timeout = setTimeout(() => {
            reject(new Error('音频加载超时'));
          }, 3000);

          audio.addEventListener('canplay', () => {
            clearTimeout(timeout);
            resolve(true);
          });

          audio.addEventListener('error', (e) => {
            clearTimeout(timeout);
            reject(e);
          });

          audio.load();
        });

        // 播放音频
        await audio.play();
        const endTime = Date.now();
        console.log(`✅ [${new Date().toLocaleTimeString()}] 铃声播放成功，耗时: ${endTime - startTime}ms，路径:`, path);
        return; // 成功播放，退出循环

      } catch (error) {
        console.warn(`❌ 路径 ${i + 1} 播放失败:`, error);
        continue; // 尝试下一个路径
      }
    }

    console.error('❌ 所有音频路径都播放失败');
  }

  // 测试播放铃声
  function testPlaySound(soundName: string) {
    playNotificationSound(soundName);
  }

  // 更新通知设置
  function updateNotificationSettings(key: string, value: any) {
    notificationSettings = {
      ...notificationSettings,
      [key]: value
    };

    // 保存到本地存储
    localStorage.setItem('notificationSettings', JSON.stringify(notificationSettings));

    // 如果启用桌面通知，请求权限
    if (key === 'enableDesktopNotification' && value) {
      requestNotificationPermission();
    }
  }

  // 请求桌面通知权限（Tauri应用）
  async function requestNotificationPermission() {
    try {
      // 使用Tauri通知API检查和请求权限
      const { isPermissionGranted, requestPermission } = await import('@tauri-apps/plugin-notification');

      let permissionGranted = await isPermissionGranted();

      if (!permissionGranted) {
        const permission = await requestPermission();
        permissionGranted = permission === 'granted';
      }

      console.log('Tauri通知权限状态:', permissionGranted);
      return permissionGranted;
    } catch (error) {
      console.warn('Tauri通知权限检查失败:', error);
      return false;
    }
  }

  // 预加载通知API以减少延迟
  let notificationAPI: any = null;

  // 在组件初始化时预加载通知API
  async function preloadNotificationAPI() {
    try {
      const api = await import('@tauri-apps/plugin-notification');
      notificationAPI = api;
      console.log('✅ 通知API预加载完成');
    } catch (error) {
      console.warn('⚠️ 通知API预加载失败:', error);
    }
  }

  // 显示桌面通知（使用直接的Tauri命令，最快速度）
  function showDesktopNotification(title: string, body: string, accountName: string) {
    if (!notificationSettings.enableDesktopNotification) return;

    const startTime = performance.now();
    console.log(`🔔 [${new Date().toLocaleTimeString()}] 立即发送桌面通知:`, title);

    // 使用静态导入的invoke函数，避免动态导入延迟
    invoke('send_desktop_notification', { title, body })
      .then(() => {
        const endTime = performance.now();
        console.log(`⚡ [${new Date().toLocaleTimeString()}] 桌面通知(Tauri命令)发送完成，耗时: ${(endTime - startTime).toFixed(2)}ms`);
      })
      .catch(error => {
        console.warn('❌ Tauri命令通知失败，尝试降级方案:', error);

        // 降级方案：使用预加载的API
        if (notificationAPI && notificationAPI.sendNotification) {
          notificationAPI.sendNotification({
            title: title,
            body: body,
            icon: 'icon.png'
          });
          const endTime = performance.now();
          console.log(`🔄 [${new Date().toLocaleTimeString()}] 桌面通知(降级)发送完成，耗时: ${(endTime - startTime).toFixed(2)}ms`);
        } else {
          // 最后的降级方案
          import('@tauri-apps/plugin-notification').then(api => {
            api.sendNotification({
              title: title,
              body: body,
              icon: 'icon.png'
            });
            const endTime = performance.now();
            console.log(`🔄 [${new Date().toLocaleTimeString()}] 桌面通知(最终降级)发送完成，耗时: ${(endTime - startTime).toFixed(2)}ms`);
          }).catch(finalError => {
            console.warn('❌ 所有通知方案都失败:', finalError);
          });
        }
      });
  }

  // 实时邮件监听状态（只有在有账户时才启用）
  $: realTimeListenerEnabled = $accounts.length > 0 && newMessagesUnlisten !== null;
  let newMessagesUnlisten: (() => void) | null = null;

  // 删除确认对话框状态
  let showDeleteConfirm = false;
  let emailToDelete: EmailMessage | null = null;
  let deleteConfirmMessage = '';

  // 移出垃圾邮件功能
  async function moveOutOfSpam(email: EmailMessage) {
    try {
      addNotification('info', '正在移出垃圾邮件...');

      // 调用API将邮件从垃圾箱移动到收件箱
      await EmailAPI.moveMessage($currentAccount!.id, email.message_id, 'SPAM', 'INBOX');

      addNotification('success', '邮件已移出垃圾箱');

      // 关闭详情面板
      selectedEmail = null;

      // 立即从当前邮件列表中移除该邮件
      messages.update(currentMessages => {
        return currentMessages.filter(msg => msg.id !== email.id);
      });

      // 跨文件夹实时同步：清除收件箱缓存，确保下次访问时显示新邮件
      if ($currentAccount) {
        EmailAPI.refreshMessages($currentAccount.id, 'INBOX').then(() => {
          console.log('收件箱缓存已刷新，邮件已移出垃圾邮件');
        }).catch(err => {
          console.warn('刷新收件箱缓存失败:', err);
        });
      }
    } catch (error) {
      console.error('移出垃圾邮件失败:', error);
      addNotification('error', `移出垃圾邮件失败: ${error}`);
    }
  }

  // 标记为垃圾邮件功能
  async function markAsSpam(email: EmailMessage) {
    try {
      addNotification('info', '正在标记为垃圾邮件...');

      // 调用API将邮件从当前文件夹移动到垃圾箱
      await EmailAPI.moveMessage($currentAccount!.id, email.message_id, currentFolder.toUpperCase(), 'SPAM');

      addNotification('success', '邮件已标记为垃圾邮件');

      // 关闭详情面板
      selectedEmail = null;

      // 立即从当前邮件列表中移除该邮件
      messages.update(currentMessages => {
        return currentMessages.filter(msg => msg.id !== email.id);
      });

      // 跨文件夹实时同步：清除垃圾邮件文件夹缓存，确保下次访问时显示新邮件
      if ($currentAccount) {
        EmailAPI.refreshMessages($currentAccount.id, 'SPAM').then(() => {
          console.log('垃圾邮件文件夹缓存已刷新，邮件已标记为垃圾邮件');
        }).catch(err => {
          console.warn('刷新垃圾邮件文件夹缓存失败:', err);
        });
      }

    } catch (error) {
      console.error('标记为垃圾邮件失败:', error);
      addNotification('error', `标记为垃圾邮件失败: ${error}`);
    }
  }

  // 还原邮件功能（从垃圾箱还原到收件箱）
  async function restoreEmail(event: Event, email: EmailMessage) {
    event.stopPropagation(); // 阻止事件冒泡，避免触发邮件选择

    try {
      addNotification('info', '正在还原邮件...');

      // 调用API将邮件从垃圾箱移动到收件箱
      await EmailAPI.moveMessage($currentAccount!.id, email.message_id, 'TRASH', 'INBOX');

      addNotification('success', '邮件已还原到收件箱');

      // 立即从当前邮件列表中移除该邮件
      messages.update(currentMessages => {
        return currentMessages.filter(msg => msg.id !== email.id);
      });

      // 刷新收件箱缓存
      if ($currentAccount) {
        try {
          // 异步刷新收件箱缓存
          EmailAPI.getMessages($currentAccount.id, 'INBOX', undefined, true).then(() => {
            console.log('收件箱缓存已刷新');
          }).catch(err => {
            console.warn('刷新收件箱缓存失败:', err);
          });
        } catch (err) {
          console.warn('启动收件箱缓存刷新失败:', err);
        }
      }

    } catch (error) {
      console.error('还原邮件失败:', error);
      addNotification('error', `还原邮件失败: ${error}`);
    }
  }

  // 标记单个邮件为已读
  async function markEmailAsRead(email: EmailMessage) {
    try {
      if (!$currentAccount) {
        addNotification('error', '未选择账户');
        return;
      }

      await EmailAPI.markAsRead($currentAccount.id, email.id, true);

      // 更新本地邮件状态
      messages.update(msgs =>
        msgs.map(msg =>
          msg.id === email.id ? { ...msg, is_read: true } : msg
        )
      );

      // 如果是当前选中的邮件，也更新选中邮件的状态
      if (selectedEmail && selectedEmail.id === email.id) {
        selectedEmail = { ...selectedEmail, is_read: true };
      }

      addNotification('success', '邮件已标记为已读');
    } catch (error) {
      console.error('标记邮件为已读失败:', error);
      addNotification('error', `标记邮件为已读失败: ${error}`);
    }
  }

  // 标记单个邮件为未读
  async function markEmailAsUnread(email: EmailMessage) {
    try {
      if (!$currentAccount) {
        addNotification('error', '未选择账户');
        return;
      }

      await EmailAPI.markAsRead($currentAccount.id, email.id, false);

      // 更新本地邮件状态
      messages.update(msgs =>
        msgs.map(msg =>
          msg.id === email.id ? { ...msg, is_read: false } : msg
        )
      );

      // 如果是当前选中的邮件，也更新选中邮件的状态
      if (selectedEmail && selectedEmail.id === email.id) {
        selectedEmail = { ...selectedEmail, is_read: false };
      }

      addNotification('success', '邮件已标记为未读');
    } catch (error) {
      console.error('标记邮件为未读失败:', error);
      addNotification('error', `标记邮件为未读失败: ${error}`);
    }
  }

  // 切换邮件星标状态
  async function toggleEmailStar(event: Event, email: EmailMessage) {
    event.stopPropagation(); // 阻止事件冒泡，避免触发邮件选择

    try {
      if (!$currentAccount) {
        addNotification('error', '未选择账户');
        return;
      }

      const newStarredState = !email.is_starred;
      await EmailAPI.markAsStarred($currentAccount.id, email.id, newStarredState);

      // 更新本地邮件状态
      messages.update(msgs =>
        msgs.map(msg =>
          msg.id === email.id ? { ...msg, is_starred: newStarredState } : msg
        )
      );

      // 如果是当前选中的邮件，也更新选中邮件的状态
      if (selectedEmail && selectedEmail.id === email.id) {
        selectedEmail = { ...selectedEmail, is_starred: newStarredState };
      }

      // 如果当前在收藏文件夹，且取消了星标，则从列表中移除
      if (currentFolder === 'starred' && !newStarredState) {
        messages.update(msgs => msgs.filter(msg => msg.id !== email.id));
      }

      addNotification('success', newStarredState ? '已添加到收藏' : '已从收藏中移除');
    } catch (error) {
      console.error('切换星标状态失败:', error);
      addNotification('error', `切换星标状态失败: ${error}`);
    }
  }

  // 回复邮件功能
  let showReplyDialog = false;
  let replySubject = '';
  let replyBody = '';
  let replyToEmail: EmailMessage | null = null;

  // 转发邮件功能
  let showForwardDialog = false;
  let forwardEmail: EmailMessage | null = null;
  let cachedForwardContent: string = ''; // 缓存转发内容，避免重复生成

  // 回复富文本编辑器
  let replyEditorElement: HTMLDivElement;
  let showReplyFontSizePicker = false;
  let showReplyColorPicker = false;

  // 保存草稿确认对话框
  let showSaveDraftConfirm = false;

  // 标记是否正在编辑草稿（用于区分新回复和编辑草稿）
  let isEditingDraft = false;
  let originalDraftContent = '';
  let currentDraftId: string | null = null; // 当前编辑的草稿ID

  function openReplyDialog(email: EmailMessage) {
    replyToEmail = email;

    // 调试信息：确认回复的邮件信息
    console.log('打开回复对话框，原始邮件信息:', {
      subject: email.subject,
      sender: email.sender,
      message_id: email.message_id,
      body_text_length: email.body_text?.length || 0,
      body_html_length: email.body_html?.length || 0,
      received_at: email.received_at
    });

    // 根据账户类型设置回复主题前缀
    const isGmail = $currentAccount?.email.includes('@gmail.com');
    const replyPrefix = isGmail ? 'Re: ' : '回复: ';

    // 检查主题是否已经有回复前缀
    const hasRePrefix = email.subject.startsWith('Re: ') || email.subject.startsWith('回复: ');
    replySubject = hasRePrefix ? email.subject : `${replyPrefix}${email.subject}`;

    replyBody = ''; // 用户输入区域初始为空
    isEditingDraft = false; // 这是新回复，不是编辑草稿
    originalDraftContent = '';
    currentDraftId = null; // 重置草稿ID
    showReplyDialog = true;
  }

  function closeReplyDialog() {
    // 检查是否有未保存的回复内容
    if (isEditingDraft) {
      // 如果是编辑草稿，只有内容发生变化时才询问保存
      if (replyBody.trim() !== originalDraftContent.trim()) {
        showSaveDraftConfirm = true;
      } else {
        // 内容没有变化，直接关闭
        forceCloseReplyDialog();
      }
    } else {
      // 如果是新回复，有内容就询问保存
      if (replyBody.trim()) {
        showSaveDraftConfirm = true;
      } else {
        // 没有内容，直接关闭
        forceCloseReplyDialog();
      }
    }
  }

  // 强制关闭回复对话框（不检查内容）
  function forceCloseReplyDialog() {
    showReplyDialog = false;
    replyToEmail = null;
    replySubject = '';
    replyBody = '';
    // 清空富文本编辑器
    if (replyEditorElement) {
      replyEditorElement.innerHTML = '';
    }
    isEditingDraft = false;
    originalDraftContent = '';
    currentDraftId = null;
  }

  // 转发邮件功能
  function openForwardDialog(email: EmailMessage) {
    forwardEmail = email;
    // 生成并缓存转发内容，避免重复调用
    cachedForwardContent = formatForwardContent(email);
    showForwardDialog = true;
  }

  function closeForwardDialog() {
    showForwardDialog = false;
    forwardEmail = null;
    cachedForwardContent = ''; // 清理缓存
  }

  // 格式化转发邮件内容
  function formatForwardContent(email: EmailMessage): string {
    const senderEmail = getSenderEmail(email.sender);
    const senderName = getSenderDisplayName(email.sender);

    // 处理收件人列表
    let recipientsList = '';
    if (email.recipients) {
      if (typeof email.recipients === 'string') {
        try {
          // 尝试解析JSON格式的收件人列表
          const parsed = JSON.parse(email.recipients);
          if (Array.isArray(parsed)) {
            // 构建标准邮箱格式：email <email>
            recipientsList = parsed.map(email => `${email} <${email}>`).join(', ');
          } else {
            // 构建标准邮箱格式：email <email>
            const emails = email.recipients.split(',').map(email => email.trim());
            recipientsList = emails.map(email => `${email} <${email}>`).join(', ');
          }
        } catch {
          // 如果不是JSON，按逗号分隔处理，构建标准邮箱格式
          const emails = email.recipients.split(',').map(email => email.trim());
          recipientsList = emails.map(email => `${email} <${email}>`).join(', ');
        }
      } else {
        recipientsList = '未知收件人';
      }
    }

    // 获取邮件内容 - 保持原始格式，避免重复的转发格式
    let originalContent = '';

    if (email.body_html && email.body_html.trim()) {
      // 优先使用HTML格式，保持原始格式
      originalContent = email.body_html.trim();
    } else if (email.body_text && email.body_text.trim()) {
      // 如果没有HTML，使用纯文本并转换换行符
      originalContent = email.body_text.trim().replace(/\n/g, '<br>');
    }

    // 如果内容包含之前的转发格式，只提取最核心的原始内容
    const forwardSeparators = [
      '发件人:',
      'From:',
      '-------- Original Message --------',
      '-----原始邮件-----',
      '---------------原始邮件---------------',
      '<hr style="display:inline-block;width:98%"',  // Outlook HTML分隔线
      '<div id="divRplyFwdMsg"'  // Outlook HTML容器
    ];

    // 查找是否包含转发分隔符，如果有，只取第一个分隔符之前的内容
    for (const separator of forwardSeparators) {
      const index = originalContent.indexOf(separator);
      if (index !== -1) {
        originalContent = originalContent.substring(0, index).trim();
        break;
      }
    }

    // 格式化转发内容 - 使用标准的水平分割线格式，减少多余换行
    const forwardHeader = `<hr style="border: none; border-top: 1px solid #ccc; margin: 15px 0;">
<div style="font-family: Arial, sans-serif; font-size: 13px;">
<p style="margin: 5px 0; color: #666; font-size: 14px;"><strong>---------- 转发邮件 ----------</strong></p>
<p style="margin: 5px 0;"><strong>发件人:</strong> ${senderName} &lt;${senderEmail}&gt;</p>
<p style="margin: 5px 0;"><strong>发送时间:</strong> ${formatEmailTime(email.received_at)}</p>
<p style="margin: 5px 0;"><strong>收件人:</strong> ${recipientsList}</p>
<p style="margin: 5px 0;"><strong>主题:</strong> ${email.subject}</p>
<br>
<div style="margin-top: 10px;">
${originalContent || '(无内容)'}
</div>
</div>`;

    return forwardHeader;
  }

  // 回复富文本编辑器功能
  function replyExecCommand(command: string, value?: string) {
    document.execCommand(command, false, value);
    replyEditorElement?.focus();
  }

  function replyFormatText(command: string) {
    replyExecCommand(command);
  }

  function setReplyFontSize(size: string) {
    replyExecCommand('fontSize', size);
    showReplyFontSizePicker = false;
  }

  function setReplyTextColor(color: string) {
    replyExecCommand('foreColor', color);
    showReplyColorPicker = false;
  }

  function insertReplyList(type: 'ul' | 'ol') {
    if (type === 'ul') {
      replyExecCommand('insertUnorderedList');
    } else {
      replyExecCommand('insertOrderedList');
    }
  }

  function insertReplyLink() {
    const url = prompt('请输入链接地址:');
    if (url) {
      replyExecCommand('createLink', url);
    }
  }

  function insertReplyImage() {
    const url = prompt('请输入图片地址:');
    if (url) {
      replyExecCommand('insertImage', url);
    }
  }

  // 获取回复编辑器内容
  function getReplyEditorContent(): string {
    return replyEditorElement?.innerHTML || '';
  }

  // 设置回复编辑器内容
  function setReplyEditorContent(content: string) {
    if (replyEditorElement) {
      replyEditorElement.innerHTML = content;
    }
  }

  // 保存回复为草稿
  async function saveReplyAsDraft() {
    if (!replyToEmail || !$currentAccount) {
      addNotification('error', '保存草稿失败：缺少必要信息');
      return;
    }

    try {
      addNotification('info', '正在保存草稿...');

      // 确定回复的收件人地址
      let replyToAddress: string;

      // 检查是否是已发送的邮件
      const isSentEmail = replyToEmail.folder.toLowerCase() === 'sent' ||
                         replyToEmail.folder.toLowerCase() === 'sentitems' ||
                         replyToEmail.folder.toLowerCase() === '已发送';

      if (isSentEmail) {
        // 对于已发送邮件，回复给原始收件人
        if (replyToEmail.recipients && replyToEmail.recipients.trim() !== '') {
          // recipients是逗号分隔的字符串，不是JSON
          const recipientsList = replyToEmail.recipients.split(',').map(email => email.trim());
          if (recipientsList.length > 0 && recipientsList[0] !== '') {
            replyToAddress = recipientsList[0]; // 使用第一个收件人
          } else {
            addNotification('error', '无法找到原始收件人');
            return;
          }
        } else {
          addNotification('error', '已发送邮件没有收件人信息');
          return;
        }
      } else {
        // 对于普通邮件，回复给发件人
        const originalSender = replyToEmail.sender;
        replyToAddress = originalSender;

        // 如果发件人格式是 "Name <email@domain.com>"，提取邮箱地址
        const emailMatch = originalSender.match(/<([^>]+)>/);
        if (emailMatch) {
          replyToAddress = emailMatch[1];
        }
      }

      // 构建草稿内容（包含原邮件引用）
      let originalContent = replyToEmail.body_text;
      if (!originalContent && replyToEmail.body_html) {
        originalContent = replyToEmail.body_html
          .replace(/<[^>]*>/g, '')
          .replace(/&nbsp;/g, ' ')
          .replace(/&lt;/g, '<')
          .replace(/&gt;/g, '>')
          .replace(/&amp;/g, '&')
          .trim();
      }

      // 草稿保存时需要保存完整内容（用户输入 + 原邮件引用）
      const quotedContent = `\n\n---------- 原始邮件 ----------\nFrom: ${replyToEmail.sender}\nSent: ${new Date(replyToEmail.received_at).toLocaleString()}\nTo: ${replyToEmail.recipients}\nSubject: ${replyToEmail.subject}\n\n${originalContent}`;
      const fullDraftBody = replyBody + quotedContent;

      // 直接调用Microsoft Graph API保存草稿
      try {
        console.log('保存草稿数据:', {
          to: [replyToAddress],
          subject: replySubject,
          body_text: fullDraftBody,
          in_reply_to: replyToEmail.message_id
        });

        // 构建草稿邮件请求
        const draftRequest = {
          account_id: $currentAccount.id,
          to: [replyToAddress],
          cc: [],
          bcc: [],
          subject: replySubject,
          body_text: fullDraftBody,
          attachments: [],
          in_reply_to: replyToEmail.message_id,
          references: replyToEmail.message_id,
          draft_id: currentDraftId || undefined // 如果是编辑现有草稿，传递草稿ID
        };

        console.log('保存草稿数据:', draftRequest);

        // 调用专门的保存草稿API
        await EmailAPI.saveDraft($currentAccount.id, draftRequest);

        // 跨文件夹实时同步：清除草稿文件夹缓存，确保下次访问时显示新草稿
        if ($currentAccount) {
          EmailAPI.refreshMessages($currentAccount.id, 'DRAFTS').then(() => {
            console.log('草稿文件夹缓存已刷新，草稿已保存');
          }).catch(err => {
            console.warn('刷新草稿文件夹缓存失败:', err);
          });
        }

        addNotification('success', '草稿已保存');
      } catch (error) {
        console.error('保存草稿失败:', error);
        addNotification('error', `保存草稿失败: ${error}`);
      }
      showSaveDraftConfirm = false;
      forceCloseReplyDialog();
    } catch (error) {
      console.error('保存草稿失败:', error);
      addNotification('error', `保存草稿失败: ${error}`);
    }
  }

  // 不保存草稿，直接关闭
  function discardReplyDraft() {
    showSaveDraftConfirm = false;
    forceCloseReplyDialog();
  }



  function openComposeDialog() {
    showComposeDialog = true;
  }

  // 从邮件内容中提取原始邮件ID（适用于草稿邮件和已发送邮件）
  function extractOriginalMessageId(email: EmailMessage): string | null {
    // 检查是否是草稿邮件或垃圾箱中的草稿
    const isDraft = email.folder.toLowerCase() === 'drafts' ||
                   email.folder.toLowerCase() === '草稿' ||
                   email.folder.toLowerCase() === 'trash' ||
                   isDraftEmail(email);

    // 检查是否是已发送的回复邮件
    const isSentReply = email.folder.toLowerCase() === 'sent' ||
                       email.folder.toLowerCase() === 'sentitems' ||
                       email.folder.toLowerCase() === '已发送';

    // 对于普通邮件，检查是否是回复邮件
    if (!isDraft && !isSentReply) {
      // 检查是否是回复邮件（通过主题判断）
      const isReplyEmail = email.subject.startsWith('Re: ') ||
                          email.subject.startsWith('回复: ') ||
                          email.subject.includes('Re:') ||
                          email.subject.includes('回复:');

      if (!isReplyEmail) {
        // 如果不是回复邮件，直接返回其ID
        return email.message_id;
      }

      // 如果是回复邮件，尝试从内容中提取原始邮件信息
      // 继续执行下面的提取逻辑
    }

    // 对于已发送的邮件，直接使用其ID作为回复目标
    // 这样用户可以"继续回复"这封已发送的邮件
    if (isSentReply) {
      return email.message_id;
    }

    // 从草稿邮件的内容中查找原始邮件的信息
    const content = email.body_text || email.body_html || '';

    // 查找原始邮件分隔符后的内容
    const separators = [
      '发件人:',  // 中文格式
      'From: ',  // 英文格式
      '-----原始邮件-----',
      '---------------原始邮件---------------',
      '\n\nOn ',  // Gmail格式
      '\n\n> ',   // 引用格式
      '<hr style="display:inline-block;width:98%"',  // Outlook HTML格式
      '<div id="divRplyFwdMsg"',  // Outlook HTML容器
    ];

    for (const separator of separators) {
      const index = content.indexOf(separator);
      if (index !== -1) {
        const originalPart = content.substring(index);

        // 尝试从原始邮件部分提取发件人信息，然后查找对应的邮件
        // 这里我们需要通过发件人和主题来查找原始邮件
        const senderMatch = originalPart.match(/发件人:\s*([^\n]+)/);
        const subjectMatch = originalPart.match(/主题:\s*([^\n]+)/);

        if (senderMatch && subjectMatch) {
          const originalSender = senderMatch[1].trim();
          const originalSubject = subjectMatch[1].trim();

          // 在当前邮件列表中查找匹配的原始邮件
          const originalEmail = $messages.find((email: EmailMessage) =>
            email.sender === originalSender &&
            email.subject === originalSubject &&
            email.folder.toLowerCase() !== 'drafts' &&
            email.folder.toLowerCase() !== 'trash'
          );

          if (originalEmail) {
            return originalEmail.message_id;
          }
        }
        break;
      }
    }

    // 如果无法从内容中提取原始邮件ID，使用当前邮件ID作为回复目标
    // 这样可以确保回复功能始终可用，即使是多层回复也能正常工作
    console.log('无法从内容中提取原始邮件ID，使用当前邮件ID作为回复目标:', email.message_id);
    return email.message_id;
  }

  // 构建完整的References链
  function buildReferencesChain(email: EmailMessage): string {
    // 对于回复邮件，References应该包含原始邮件的Message-ID
    // 如果原始邮件本身也是回复，理想情况下应该包含整个链
    // 但由于我们没有存储完整的邮件头，这里只使用原始邮件的Message-ID
    return email.message_id || email.id;
  }

  async function sendReply() {
    if (!replyToEmail || !$currentAccount) {
      addNotification('error', '回复信息不完整');
      return;
    }

    try {
      addNotification('info', '正在发送回复...');

      // 确定回复的收件人地址
      let replyToAddress: string;

      // 检查是否是已发送的邮件
      const isSentEmail = replyToEmail.folder.toLowerCase() === 'sent' ||
                         replyToEmail.folder.toLowerCase() === 'sentitems' ||
                         replyToEmail.folder.toLowerCase() === '已发送';

      if (isSentEmail) {
        // 对于已发送邮件，回复给原始收件人
        if (replyToEmail.recipients && replyToEmail.recipients.trim() !== '') {
          // recipients是逗号分隔的字符串，不是JSON
          const recipientsList = replyToEmail.recipients.split(',').map(email => email.trim());
          if (recipientsList.length > 0 && recipientsList[0] !== '') {
            replyToAddress = recipientsList[0]; // 使用第一个收件人
          } else {
            addNotification('error', '无法找到原始收件人');
            return;
          }
        } else {
          addNotification('error', '已发送邮件没有收件人信息');
          return;
        }
      } else {
        // 对于普通邮件，回复给发件人
        const originalSender = replyToEmail.sender;
        replyToAddress = originalSender;

        // 如果发件人格式是 "Name <email@domain.com>"，提取邮箱地址
        const emailMatch = originalSender.match(/<([^>]+)>/);
        if (emailMatch) {
          replyToAddress = emailMatch[1];
        }
      }

      // 获取用户输入的回复内容
      const replyHtmlContent = getReplyEditorContent();
      const replyTextContent = replyEditorElement?.textContent || '';

      // 根据邮箱类型决定是否添加引用
      const isOutlook = $currentAccount?.email.includes('@outlook.com') ||
                       $currentAccount?.email.includes('@hotmail.com') ||
                       $currentAccount?.email.includes('@live.com');

      let fullReplyBody, fullReplyHtmlContent;

      if (isOutlook) {
        // Outlook使用Graph API回复，API会自动处理引用格式
        // 这是理想的情况：API自动处理一切
        fullReplyBody = replyTextContent;
        fullReplyHtmlContent = replyHtmlContent;
      } else {
        // QQ邮箱等使用SMTP协议，需要模拟现代邮件客户端的行为
        // 现代邮件客户端在回复时会自动添加标准的引用格式
        // 我们在这里模拟这个行为，让用户体验保持一致

        // 获取原始邮件内容，避免嵌套引用
        let originalContent = replyToEmail.body_text || '';

        // 如果内容包含之前的引用格式，只提取最核心的原始内容
        const quoteSeparators = [
          '发件人:',
          'From:',
          '-------- Original Message --------',
          '-----原始邮件-----',
          '---------------原始邮件---------------',
          '<hr style="display:inline-block;width:98%"',
          '<div id="divRplyFwdMsg"'
        ];

        // 查找是否包含引用分隔符，如果有，只取第一个分隔符之前的内容
        for (const separator of quoteSeparators) {
          const index = originalContent.indexOf(separator);
          if (index !== -1) {
            originalContent = originalContent.substring(0, index).trim();
            break;
          }
        }
        const originalDate = new Date(replyToEmail.received_at).toLocaleString('zh-CN', {
          year: 'numeric',
          month: 'long',
          day: 'numeric',
          hour: '2-digit',
          minute: '2-digit'
        });
        // 只使用HTML格式的引用，不使用文本版本的引用
        const currentAccountEmail = $currentAccount?.email || '';
        const originalSender = replyToEmail.sender || '';

        // 调试信息：检查发件人数据
        console.log('调试 - 原始发件人数据:', originalSender);
        console.log('调试 - 完整邮件对象:', replyToEmail);

        // 构建HTML格式的引用 - 使用Outlook标准格式
        const quotedOriginalHtml = `
<div style="font-family: Calibri, Arial, Helvetica, sans-serif; font-size: 11pt;">
${replyHtmlContent}
</div>
<br>
<hr style="display:inline-block;width:98%" tabindex="-1">
<div id="divRplyFwdMsg" dir="ltr">
<font face="Calibri, sans-serif" style="font-size:11pt" color="#000000">
<b>发件人:</b> ${originalSender.replace(/</g, '&lt;').replace(/>/g, '&gt;')}<br>
<b>发送时间:</b> ${originalDate}<br>
<b>收件人:</b> ${currentAccountEmail} &lt;${currentAccountEmail}&gt;<br>
<b>主题:</b> ${replyToEmail.subject}<br>
</font>
<br>
<div style="font-family: Calibri, Arial, Helvetica, sans-serif; font-size: 11pt;">
${originalContent.replace(/\n/g, '<br>')}
</div>
</div>`;

        fullReplyBody = replyTextContent;
        fullReplyHtmlContent = quotedOriginalHtml;
      }

      // 获取正确的原始邮件ID
      const originalMessageId = extractOriginalMessageId(replyToEmail);

      if (!originalMessageId) {
        addNotification('error', '无法找到原始邮件，无法发送回复');
        return;
      }

      // 调试信息：检查发送的内容
      console.log('发送邮件请求内容:');
      console.log('body_text:', fullReplyBody);
      console.log('body_html:', fullReplyHtmlContent);
      console.log('邮箱类型:', isOutlook ? 'Outlook' : 'QQ/其他');

      await EmailAPI.sendEmail($currentAccount.id, {
        account_id: $currentAccount.id,
        to: [replyToAddress],
        cc: [],
        bcc: [],
        subject: replySubject,
        body_text: fullReplyBody, // 包含原始邮件引用的完整文本内容
        body_html: fullReplyHtmlContent?.trim() ? fullReplyHtmlContent : undefined, // 包含原始邮件引用的完整HTML内容
        attachments: [],
        in_reply_to: originalMessageId, // 使用提取的原始邮件ID
        references: buildReferencesChain(replyToEmail) // 构建完整的邮件引用链
      });

      addNotification('success', '回复已发送');

      // 关闭回复对话框
      forceCloseReplyDialog();

      // 关闭邮件详情页
      selectedEmail = null;

      // 邮件发送成功，如果在已发送文件夹就延迟刷新，等待后台任务完成
      if (currentFolder === 'sent') {
        // 延迟5秒刷新，确保后台IMAP任务完成
        setTimeout(() => {
          console.log('延迟刷新已发送文件夹，等待后台任务完成');
          loadMessages($currentAccount?.id || '', 'SENT', true); // 强制刷新已发送文件夹
        }, 5000);
      }
    } catch (error) {
      console.error('发送回复失败:', error);
      addNotification('error', `发送回复失败: ${error}`);
    }
  }
  let isInTrashFolder = false;

  // 邮件标签缓存（保留，用于UI性能优化）
  let emailTagsCache: Map<string, EmailTag[]> = new Map();

  // 分组展开状态
  let expandedGroups: Record<string, boolean> = {};

  // 邮箱提供商列表
  let providers: EmailProviderConfig[] = [];

  // 切换分组展开/折叠
  function toggleGroup(groupName: keyof typeof expandedGroups) {
    expandedGroups[groupName] = !expandedGroups[groupName];
  }



  function switchViewMode(mode: 'list' | 'timeline' | 'chat') {
    viewMode = mode;
  }

  function selectEmail(email: any) {
    console.log('selectEmail called, currentFolder:', currentFolder, 'email:', email.subject);

    // 如果是草稿文件夹，直接编辑草稿
    if (currentFolder === 'drafts') {
      console.log('草稿文件夹，直接编辑草稿');
      editDraftMessage(email);
      return; // 草稿模式不设置selectedEmail
    } else {
      console.log('普通文件夹，显示详情');
      // 普通邮件，显示详情
      selectedEmail = email;
      // 调用自适应缩放函数
      adjustEmailContentScale();
    }
  }

  // 编辑草稿邮件
  function editDraftMessage(email: EmailMessage) {
    console.log('editDraftMessage called:', email);

    // 检查是否是回复草稿（通过主题判断）
    const isReplyDraft = email.subject.startsWith('Re: ') || email.subject.startsWith('回复: ');

    if (isReplyDraft) {
      // 如果是回复草稿，打开回复对话框
      console.log('这是回复草稿，打开回复对话框');
      openReplyDraftDialog(email);
    } else {
      // 如果是普通草稿，打开编辑草稿对话框
      console.log('这是普通草稿，打开编辑草稿对话框');
      const draftData = parseDraftMessage(email);
      showComposeDialog = true;
      window.dispatchEvent(new CustomEvent('editDraft', {
        detail: draftData
      }));
    }
  }

  // 打开回复草稿对话框
  function openReplyDraftDialog(draftEmail: EmailMessage) {
    // 创建一个虚拟的原邮件对象用于回复对话框
    // 从草稿邮件的内容中提取原邮件信息
    const originalEmailInfo = extractOriginalEmailFromDraft(draftEmail);

    // 设置回复对话框的状态
    replyToEmail = originalEmailInfo;
    replySubject = draftEmail.subject;

    // 从草稿内容中提取用户的回复部分
    replyBody = extractUserReplyFromDraft(draftEmail);

    // 标记这是编辑草稿
    isEditingDraft = true;
    originalDraftContent = replyBody; // 保存原始草稿内容
    currentDraftId = draftEmail.message_id; // 保存草稿ID

    showReplyDialog = true;
  }

  // 从草稿邮件中提取原邮件信息
  function extractOriginalEmailFromDraft(draftEmail: EmailMessage): EmailMessage {
    // 尝试从收件人信息中获取原发件人
    let originalSender = '';
    try {
      if (draftEmail.recipients) {
        const recipients = JSON.parse(draftEmail.recipients);
        originalSender = recipients[0] || '';
      }
    } catch (e) {
      originalSender = draftEmail.recipients || '';
    }

    // 创建一个虚拟的原邮件对象
    return {
      id: 'virtual-original-' + draftEmail.id,
      account_id: draftEmail.account_id,
      message_id: 'virtual-original-' + draftEmail.message_id,
      subject: draftEmail.subject.replace(/^(Re: |回复: )/, ''), // 移除回复前缀
      sender: originalSender,
      recipients: draftEmail.sender || '', // 草稿的发件人就是原邮件的收件人
      cc: '',
      bcc: '',
      body_text: '[原邮件内容]', // 虚拟的原邮件内容占位符
      body_html: '',
      folder: 'INBOX',
      is_read: true,
      is_starred: false,
      is_deleted: false,
      received_at: draftEmail.received_at,
      created_at: draftEmail.created_at || draftEmail.received_at,
      updated_at: draftEmail.updated_at || draftEmail.received_at
    };
  }

  // 从草稿内容中提取用户的回复部分
  function extractUserReplyFromDraft(draftEmail: EmailMessage): string {
    // 获取草稿的内容
    let content = '';
    if (draftEmail.body_text && draftEmail.body_text.trim()) {
      content = draftEmail.body_text.trim();
    } else if (draftEmail.body_html && draftEmail.body_html.trim()) {
      // 从HTML中提取纯文本
      content = draftEmail.body_html
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
    }

    // 根据我们的草稿格式提取用户回复内容
    const separators = [
      '\n\n---------- 原始邮件 ----------',  // 我们的新格式
      '\n\nFrom: ',                        // 标准格式
      '-----原始邮件-----',                 // 中文格式
      '--- 原始邮件 ---',                   // 其他中文格式
      '发件人:',                           // 简单中文格式
      '发送时间:',                         // 时间格式
      'From:',                            // 英文格式
      'Sent:'                             // 发送时间
    ];

    for (const separator of separators) {
      const index = content.indexOf(separator);
      if (index !== -1) {
        return content.substring(0, index).trim();
      }
    }

    // 如果没有找到分隔符，返回全部内容
    return content;
  }

  // 解析草稿邮件内容
  function parseDraftMessage(email: EmailMessage) {
    let to: string[] = [];
    let cc: string[] = [];
    let bcc: string[] = [];

    try {
      // recipients是JSON字符串，需要解析
      if (email.recipients) {
        to = JSON.parse(email.recipients);
      }
      if (email.cc) {
        cc = JSON.parse(email.cc);
      }
      if (email.bcc) {
        bcc = JSON.parse(email.bcc);
      }
    } catch (e) {
      console.warn('解析草稿邮件收件人信息失败:', e);
      // 如果JSON解析失败，尝试按逗号分割
      to = email.recipients ? email.recipients.split(',').map((email: string) => email.trim()) : [];
      cc = email.cc ? email.cc.split(',').map((email: string) => email.trim()) : [];
      bcc = email.bcc ? email.bcc.split(',').map((email: string) => email.trim()) : [];
    }

    // 处理邮件内容，优先使用纯文本，如果没有则从HTML提取
    let bodyContent = '';
    if (email.body_text && email.body_text.trim()) {
      bodyContent = email.body_text.trim();
    } else if (email.body_html && email.body_html.trim()) {
      // 从HTML中提取纯文本
      bodyContent = email.body_html
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
    }

    return {
      to,
      cc,
      bcc,
      subject: email.subject || '',
      body: bodyContent,
      isDraft: true,
      draftId: email.id
    };
  }

  function addAccount() {
    showAccountDialog.set(true);
  }

  // 快速添加指定提供商的账户
  function quickAddProvider(provider: EmailProviderConfig) {
    // 设置预选的提供商，然后打开对话框
    preselectedProvider.set(provider);
    showAccountDialog.set(true);
    addNotification('info', `准备添加 ${provider.name} 账户`);
  }

  // 获取提供商图标路径
  function getProviderIconPath(providerName: string): string {
    // 在生产环境中，静态资源会被复制到根目录
    const iconMap: Record<string, string> = {
      // 显示名称映射
      'Gmail': '/guge.png',
      'Outlook': '/Outlook.png',
      'QQ邮箱': '/QQ.png',
      '163邮箱': '/163.png',
      '126邮箱': '/126.png',
      'Yahoo邮箱': '/Yahoo.png',
      'iCloud': '/cloud.png',
      // 内部标识符映射（用于账户分组）
      'guge': '/guge.png',
      'QQ': '/QQ.png',
      '163': '/163.png',
      '126': '/126.png',
      'sina': '/Yahoo.png',
      'yahoo': '/Yahoo.png',
      'other': '/guge.png'
    };
    return iconMap[providerName] || '/guge.png';
  }

  // 文件夹切换防抖
  let folderSwitchTimeout: number | null = null;

  // 文件夹工具栏展开/收起状态
  let isFolderToolbarExpanded = false;

  function setCurrentFolder(folder: 'inbox' | 'sent' | 'drafts' | 'spam' | 'trash' | 'starred') {
    // 清除之前的定时器
    if (folderSwitchTimeout) {
      clearTimeout(folderSwitchTimeout);
    }

    // 立即更新UI状态
    currentFolder = folder;

    // 清空当前邮件列表，显示加载状态
    messages.set([]);
    isLoadingMessages = true;

    if ($currentAccount) {
      // 映射前端文件夹名称到后端标准名称
      const folderMapping: Record<string, string> = {
        'inbox': 'INBOX',
        'sent': 'SENT',
        'drafts': 'DRAFTS', // 前端使用复数形式
        'spam': 'SPAM',
        'trash': 'TRASH',
        'starred': 'STARRED' // 收藏邮件（虚拟文件夹）
      };

      const mappedFolder = folderMapping[folder] || folder.toUpperCase();

      // 使用短暂延迟来防抖，避免快速切换时的冲突
      folderSwitchTimeout = setTimeout(() => {
        const startTime = Date.now();
        console.log(`切换到文件夹: ${folder} (${mappedFolder}) - 开始时间: ${startTime}`);

        if (folder === 'starred') {
          // 收藏文件夹特殊处理：显示所有已收藏的邮件
          loadStarredMessages($currentAccount.id);
        } else {
          // 所有文件夹都使用统一的缓存优先策略
          loadMessages($currentAccount.id, mappedFolder, false).then(() => {
            const endTime = Date.now();
            console.log(`文件夹 ${folder} 加载完成，耗时: ${endTime - startTime}ms`);
          }).catch(err => {
            const endTime = Date.now();
            console.error(`文件夹 ${folder} 加载失败，耗时: ${endTime - startTime}ms, 错误:`, err);
          });
        }
        folderSwitchTimeout = null;
      }, 100); // 100ms防抖延迟
    }
  }

  // 防重复调用的标记 - 使用Map来跟踪每个文件夹的加载状态
  let loadingStates = new Map<string, boolean>();
  let isInitialLoadComplete = false;
  let isClearingCache = false; // 清空缓存期间的标记

  // 加载邮件（直接从SQLite数据库）
  async function loadMessages(accountId: string, folder = 'INBOX', forceRefresh = false) {
    if (!accountId) return;

    // 为每个文件夹单独跟踪加载状态
    const loadingKey = `${accountId}-${folder}`;

    // 防止同一文件夹的重复调用，但允许不同文件夹并行加载
    if (loadingStates.get(loadingKey) && !forceRefresh) {
      console.log(`文件夹 ${folder} 正在加载中，跳过重复调用`);
      return;
    }

    // 设置当前文件夹的加载状态
    loadingStates.set(loadingKey, true);
    isLoadingMessages = true;
    loadingStep = 0;
    console.log('开始加载 - loadingStep设置为:', loadingStep);

    try {
      // 让用户看到0%的初始状态
      await new Promise(resolve => setTimeout(resolve, 200));

      // 步骤1: 连接邮箱服务器
      loadingStep = 1;
      await new Promise(resolve => setTimeout(resolve, 400)); // 短暂延迟让用户看到步骤

      // 步骤2: 获取邮件列表
      loadingStep = 2;
      const emailMessages = await EmailAPI.getMessages(accountId, folder, undefined, forceRefresh);

      // 步骤3: 整理邮件内容
      loadingStep = 3;
      await new Promise(resolve => setTimeout(resolve, 200));

      // 步骤4: 完成加载
      loadingStep = 4;

      // 只有当前文件夹的邮件才更新显示（避免文件夹切换时的闪烁）
      const currentLoadingKey = `${$currentAccount?.id}-${getFolderFromCurrentFolder()}`;
      if (loadingKey === currentLoadingKey) {
        messages.set(emailMessages);



        if (forceRefresh) {
          addNotification('success', `已刷新 ${emailMessages.length} 封邮件`);
        }

        console.log(`从SQLite数据库加载了 ${emailMessages.length} 封邮件 (文件夹: ${folder})`);
      } else {
        console.log(`文件夹已切换，跳过更新显示 (加载的: ${folder}, 当前: ${getFolderFromCurrentFolder()})`);
      }

      // 标记初始加载完成并启动实时监听
      if (!isInitialLoadComplete && !forceRefresh) {
        isInitialLoadComplete = true;
        console.log('初始邮件加载完成，通知后端启动实时监听');

        // 通知后端启动实时监听
        try {
          await EmailAPI.notifyInitialLoadComplete();
          console.log('实时邮件监听启动成功');
        } catch (error) {
          console.error('启动实时邮件监听失败:', error);
        }
      }
    } catch (error) {
      console.error('加载邮件失败:', error);
      addNotification('error', `加载邮件失败: ${error}`);
    } finally {
      // 清除当前文件夹的加载状态
      loadingStates.set(loadingKey, false);

      // 只有当没有其他文件夹在加载时才清除全局加载状态
      const hasAnyLoading = Array.from(loadingStates.values()).some(loading => loading);
      if (!hasAnyLoading) {
        isLoadingMessages = false;
      }
    }
  }

  // 获取当前文件夹对应的后端文件夹名称
  function getFolderFromCurrentFolder(): string {
    const folderMapping: Record<string, string> = {
      'inbox': 'INBOX',
      'sent': 'SENT',
      'drafts': 'DRAFTS',
      'spam': 'SPAM',
      'trash': 'TRASH',
      'starred': 'STARRED'
    };
    return folderMapping[currentFolder] || currentFolder.toUpperCase();
  }

  // 加载收藏邮件（超级优化版：直接从数据库查询）
  async function loadStarredMessages(accountId: string) {
    const loadingKey = `${accountId}-STARRED`;

    // 设置当前文件夹的加载状态
    loadingStates.set(loadingKey, true);
    isLoadingMessages = true;
    loadingStep = 0;

    try {
      console.log('开始加载收藏邮件...');
      const startTime = Date.now();

      // 让用户看到0%的初始状态
      await new Promise(resolve => setTimeout(resolve, 150));

      // 步骤1: 连接邮箱服务器
      loadingStep = 1;
      await new Promise(resolve => setTimeout(resolve, 300));

      // 步骤2: 获取收藏邮件
      loadingStep = 2;
      const starredMessages = await EmailAPI.getStarredMessages(accountId);

      // 步骤3: 整理收藏内容
      loadingStep = 3;
      await new Promise(resolve => setTimeout(resolve, 150));

      // 步骤4: 完成加载
      loadingStep = 4;

      const loadTime = Date.now() - startTime;
      console.log(`收藏邮件加载完成: ${starredMessages.length} 封邮件，耗时 ${loadTime}ms`);

      // 只有当前文件夹是收藏时才更新显示
      if (currentFolder === 'starred') {
        messages.set(starredMessages);
        console.log(`显示 ${starredMessages.length} 封收藏邮件`);
      }

    } catch (error) {
      console.error('加载收藏邮件失败:', error);
      addNotification('error', `加载收藏邮件失败: ${error}`);
    } finally {
      // 清除当前文件夹的加载状态
      loadingStates.set(loadingKey, false);

      // 只有当没有其他文件夹在加载时才清除全局加载状态
      const hasAnyLoading = Array.from(loadingStates.values()).some(loading => loading);
      if (!hasAnyLoading) {
        isLoadingMessages = false;
      }
    }
  }

  // 刷新邮件（强制从服务器获取）
  async function refreshMessages() {
    if (!$currentAccount) return;
    await loadMessages($currentAccount.id, currentFolder.toUpperCase(), true);

    // 强制刷新后重新启动实时监听
    try {
      await EmailAPI.startAutoSync();
      console.log('强制刷新后重新启动实时监听');
    } catch (error) {
      console.error('重新启动实时监听失败:', error);
    }
  }

  // 全部标记为已读
  async function markAllAsRead() {
    if (!$currentAccount) return;

    try {
      // 如果邮件列表为空，先尝试加载邮件
      if (!$messages || $messages.length === 0) {
        addNotification('info', '正在加载邮件，请稍后再试...');
        await loadMessages($currentAccount.id, currentFolder.toUpperCase());

        // 加载后再次检查
        if (!$messages || $messages.length === 0) {
          addNotification('info', '没有邮件可以标记');
          return;
        }
      }

      // 获取所有未读邮件
      const unreadMessages = $messages.filter(message => !message.is_read);

      if (unreadMessages.length === 0) {
        addNotification('info', '没有未读邮件');
        return;
      }

      // 逐个标记为已读
      let successCount = 0;
      for (const message of unreadMessages) {
        try {
          await EmailAPI.markAsRead($currentAccount.id, message.id, true);
          successCount++;
        } catch (error) {
          console.error(`标记邮件 ${message.id} 为已读失败:`, error);
        }
      }

      // 更新本地状态
      messages.update(msgs =>
        msgs.map(msg => ({ ...msg, is_read: true }))
      );



      addNotification('success', `已标记 ${successCount} 封邮件为已读`);
    } catch (error) {
      console.error('全部标记为已读失败:', error);
      addNotification('error', `操作失败: ${error}`);
    }
  }



  // 显示删除确认对话框
  function showDeleteConfirmDialog(event: Event, email: EmailMessage) {
    event.stopPropagation(); // 阻止事件冒泡，避免触发邮件选择

    if (!$currentAccount) {
      addNotification('error', '请先选择邮箱账户');
      return;
    }

    // 设置对话框状态
    emailToDelete = email;
    isInTrashFolder = currentFolder.toLowerCase().includes('trash') ||
                     currentFolder.toLowerCase().includes('deleted') ||
                     currentFolder.toLowerCase().includes('垃圾');

    deleteConfirmMessage = isInTrashFolder
      ? '确定要永久删除这封邮件吗？此操作无法撤销。'
      : '确定要删除这封邮件吗？邮件将移动到垃圾箱。';

    showDeleteConfirm = true;
  }

  // 确认删除邮件
  async function confirmDeleteEmail() {
    if (!emailToDelete || !$currentAccount) return;

    try {
      const success = await EmailAPI.deleteMessage(
        $currentAccount.id,
        emailToDelete.message_id,
        currentFolder.toUpperCase()
      );

      if (success) {
        if (isInTrashFolder) {
          // 如果是在垃圾箱中永久删除，从列表中移除
          messages.update(msgs => msgs.filter(msg => msg.id !== emailToDelete!.id));
          addNotification('success', '邮件已永久删除');
        } else {
          // 如果是移动到垃圾箱，从当前文件夹列表中移除邮件
          const deletedEmail = emailToDelete;
          messages.update(msgs => msgs.filter(msg => msg.id !== deletedEmail.id));

          // 跨文件夹实时同步：清除垃圾箱缓存，确保下次访问时显示新邮件
          if ($currentAccount) {
            EmailAPI.refreshMessages($currentAccount.id, 'TRASH').then(() => {
              console.log('垃圾箱缓存已刷新，邮件已移动到垃圾箱');
            }).catch(err => {
              console.warn('刷新垃圾箱缓存失败:', err);
            });
          }

          addNotification('success', '邮件已移动到垃圾箱');
        }



        // 如果当前显示的是被删除的邮件详情，关闭详情面板
        if (selectedEmail && selectedEmail.id === emailToDelete.id) {
          selectedEmail = null;
        }
      } else {
        addNotification('error', '删除邮件失败');
      }
    } catch (error) {
      console.error('删除邮件失败:', error);

      // 检查是否是404错误（邮件已在其他客户端删除）
      const errorMessage = String(error);
      if (errorMessage.includes('404') || errorMessage.includes('Not Found') || errorMessage.includes('not found')) {
        addNotification('warning', '该邮件已在其他客户端删除，请刷新邮件列表');

        // 自动从本地列表中移除该邮件
        if (emailToDelete) {
          messages.update(msgs => msgs.filter(msg => msg.id !== emailToDelete!.id));

          // 如果当前显示的是被删除的邮件详情，关闭详情面板
          if (selectedEmail && selectedEmail.id === emailToDelete.id) {
            selectedEmail = null;
          }
        }
      } else {
        addNotification('error', `删除邮件失败: ${error}`);
      }
    } finally {
      // 关闭对话框
      showDeleteConfirm = false;
      emailToDelete = null;
    }
  }

  // 取消删除
  function cancelDelete() {
    showDeleteConfirm = false;
    emailToDelete = null;
  }

  // 清空缓存进度状态
  let clearCacheProgress = 0;
  let clearCacheStep = '';
  let showClearCacheProgress = false;

  // 清空SQLite邮件缓存
  async function clearEmailCache() {
    try {
      isClearingCache = true;
      showClearCacheProgress = true;
      clearCacheProgress = 0;
      clearCacheStep = '初始化清空操作...';

      // 模拟更细致的进度步骤
      await new Promise(resolve => setTimeout(resolve, 300));
      clearCacheProgress = 10;
      clearCacheStep = '停止实时监听...';

      await new Promise(resolve => setTimeout(resolve, 300));
      clearCacheProgress = 20;
      clearCacheStep = '清空界面显示...';

      // 立即清空界面显示的邮件
      messages.set([]);
      emailTagsCache.clear();

      await new Promise(resolve => setTimeout(resolve, 300));
      clearCacheProgress = 30;
      clearCacheStep = '连接数据库...';

      await new Promise(resolve => setTimeout(resolve, 300));
      clearCacheProgress = 40;
      clearCacheStep = '正在删除缓存数据...';

      const result = await EmailAPI.clearEmailCache();

      clearCacheProgress = 60;
      clearCacheStep = '缓存清空完成...';
      addNotification('success', result);

      await new Promise(resolve => setTimeout(resolve, 300));
      clearCacheProgress = 70;
      clearCacheStep = '准备重新连接邮箱...';

      await new Promise(resolve => setTimeout(resolve, 300));
      clearCacheProgress = 80;
      clearCacheStep = '正在重新获取邮件...';

      // 重新加载当前账户的邮件
      if ($currentAccount) {
        await loadMessages($currentAccount.id, currentFolder.toUpperCase());

        clearCacheProgress = 90;
        clearCacheStep = '正在启动实时监听...';

        // 等待一段时间确保后端重新初始化完成，然后明确启动实时监听
        await new Promise(resolve => setTimeout(resolve, 2000));

        try {
          await EmailAPI.startAutoSync();
          console.log('清空缓存后重新启动实时监听成功');
        } catch (error) {
          console.warn('重新启动实时监听失败:', error);
        }
      }

      await new Promise(resolve => setTimeout(resolve, 300));
      clearCacheProgress = 95;
      clearCacheStep = '完成初始化...';

      await new Promise(resolve => setTimeout(resolve, 300));
      clearCacheProgress = 100;
      clearCacheStep = '缓存清空完成！';
      addNotification('success', '缓存清空完成！');

      // 等待一段时间显示完成状态，然后隐藏进度条
      setTimeout(() => {
        showClearCacheProgress = false;
        isClearingCache = false;
      }, 1500);
    } catch (error) {
      console.error('清空缓存失败:', error);
      addNotification('error', `清空缓存失败: ${error}`);
      showClearCacheProgress = false;
      isClearingCache = false;
    }
  }



  // 处理背景点击
  function handleOverlayClick(event: MouseEvent) {
    if (event.target === event.currentTarget) {
      cancelDelete();
    }
  }

  // 处理对话框键盘事件
  function handleDialogKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      cancelDelete();
    }
  }

  // 当对话框显示时自动聚焦
  function focusDialog(node: HTMLElement) {
    node.focus();
    return {
      destroy() {
        // 清理函数
      }
    };
  }

  // 获取邮件标签
  async function getEmailTags(email: EmailMessage): Promise<EmailTag[]> {
    const cacheKey = email.id;

    // 检查缓存
    if (emailTagsCache.has(cacheKey)) {
      return emailTagsCache.get(cacheKey)!;
    }

    try {
      const tags = await EmailAPI.analyzeEmailTags(
        email.subject || '',
        email.body_text || ''
      );

      // 缓存结果
      emailTagsCache.set(cacheKey, tags);
      return tags;
    } catch (error) {
      console.error('分析邮件标签失败:', error);
      // 返回默认标签
      return [{
        text: 'INBOX',
        tag_type: 'Folder',
        color: '#95a5a6'
      }];
    }
  }

  // 监听当前账户变化
  let lastAccountId = '';
  $: if ($currentAccount && $currentAccount.id !== lastAccountId) {
    lastAccountId = $currentAccount.id;
    // 清除标签缓存
    emailTagsCache.clear();
    // 加载新账户的邮件（优先使用缓存，不强制刷新）
    loadMessages($currentAccount.id, currentFolder.toUpperCase(), false);
  }

  // 按邮件服务商分组账户
  $: groupedAccounts = $accounts.reduce((groups, account) => {
    const provider = getEmailProvider(account.email);
    if (!groups[provider]) {
      groups[provider] = [];
    }
    groups[provider].push(account);
    return groups;
  }, {} as Record<string, typeof $accounts>);

  // 获取邮件服务商
  function getEmailProvider(email: string): string {
    const domain = email.split('@')[1]?.toLowerCase();
    if (domain?.includes('gmail')) return 'guge';
    if (domain?.includes('qq')) return 'QQ';
    if (domain?.includes('outlook') || domain?.includes('hotmail') || domain?.includes('live')) return 'Outlook';
    if (domain?.includes('163')) return '163';
    if (domain?.includes('126')) return '126';
    if (domain?.includes('sina')) return 'sina';
    if (domain?.includes('yahoo')) return 'yahoo';
    return 'other';
  }

  // 获取服务商显示名称
  function getProviderDisplayName(provider: string): string {
    const names: Record<string, string> = {
      'guge': 'Gmail',
      'QQ': 'QQ邮箱',
      'Outlook': 'Outlook',
      '163': '163邮箱',
      '126': '126邮箱',
      'sina': '新浪邮箱',
      'yahoo': 'Yahoo邮箱',
      'other': '其他邮箱'
    };
    return names[provider] || provider;
  }

  // 选择账户
  function selectAccount(account: any) {
    currentAccount.set(account);
  }

  // 格式化邮件发送时间 - 显示完整的年月日和秒
  function formatEmailTime(dateString: string): string {
    const date = new Date(dateString);

    // 格式化为：2025-06-22 14:30:45
    const year = date.getFullYear();
    const month = String(date.getMonth() + 1).padStart(2, '0');
    const day = String(date.getDate()).padStart(2, '0');
    const hours = String(date.getHours()).padStart(2, '0');
    const minutes = String(date.getMinutes()).padStart(2, '0');
    const seconds = String(date.getSeconds()).padStart(2, '0');

    return `${year}-${month}-${day} ${hours}:${minutes}:${seconds}`;
  }

  // 检测邮件是否是草稿邮件（通过内容特征判断）
  function isDraftEmail(email: EmailMessage): boolean {
    // 检查邮件内容是否包含草稿特征
    const content = email.body_text || email.body_html || '';

    // 检查是否包含回复格式的分隔符
    const hasReplyFormat = content.includes('-----原始邮件-----') ||
                          content.includes('发件人:') ||
                          content.includes('发送时间:') ||
                          content.includes('收件人:') ||
                          content.includes('主题:') ||
                          content.includes('From:') ||
                          content.includes('Sent:') ||
                          content.includes('To:') ||
                          content.includes('Subject:');

    // 额外检查：如果发件人是空的或者只有邮箱地址，也可能是草稿
    const senderIsEmpty = !email.sender || email.sender.trim() === '';
    const senderIsJustEmail = email.sender && email.sender.includes('@') && !email.sender.includes('<');

    return hasReplyFormat || senderIsEmpty || !!senderIsJustEmail;
  }

  // 获取正确的发件人信息（不修改草稿邮件的原始信息）
  function getCorrectSender(email: EmailMessage): string {
    // 直接返回原始发件人信息，不做任何修改
    return email.sender || '未知发件人';
  }

  // 获取原始邮件的收件人信息
  function getOriginalRecipients(email: EmailMessage): string {
    // 检查是否是已发送邮件
    const isSentEmail = email.folder.toLowerCase() === 'sent' ||
                       email.folder.toLowerCase() === 'sentitems' ||
                       email.folder.toLowerCase() === '已发送';

    if (isSentEmail) {
      // 已发送邮件：显示原始收件人
      if (email.recipients && email.recipients.trim() !== '') {
        // recipients是逗号分隔的字符串
        const recipientsList = email.recipients.split(',').map(email => email.trim());
        return recipientsList.join(', ');
      } else {
        return '未知收件人';
      }
    } else {
      // 收到的邮件：显示当前用户
      if ($currentAccount) {
        const userEmail = $currentAccount.email;
        const userName = userEmail.split('@')[0];
        return `${userName} <${userEmail}>`;
      }
      return '你';
    }
  }

  // 获取回复邮件的收件人地址（用于发送）
  function getReplyToAddress(email: EmailMessage): string {
    // 检查是否是已发送邮件
    const isSentEmail = email.folder.toLowerCase() === 'sent' ||
                       email.folder.toLowerCase() === 'sentitems' ||
                       email.folder.toLowerCase() === '已发送';

    if (isSentEmail) {
      // 已发送邮件：回复给原始收件人
      if (email.recipients && email.recipients.trim() !== '') {
        // recipients是逗号分隔的字符串，取第一个收件人
        const recipientsList = email.recipients.split(',').map(email => email.trim());
        if (recipientsList.length > 0 && recipientsList[0] !== '') {
          return recipientsList[0];
        }
      }
      return '未知收件人';
    } else {
      // 收到的邮件：回复给发件人
      const originalSender = email.sender;

      // 如果发件人格式是 "Name <email@domain.com>"，提取邮箱地址
      const emailMatch = originalSender.match(/<([^>]+)>/);
      if (emailMatch) {
        return emailMatch[1];
      }

      return originalSender;
    }
  }

  // 获取邮件详情显示的内容（处理草稿邮件的特殊格式）
  function getEmailDetailContent(email: EmailMessage): string {
    // 检查是否是草稿邮件
    const isDraft = email.folder.toLowerCase() === 'drafts' ||
                   email.folder.toLowerCase() === '草稿' ||
                   email.folder.toUpperCase() === 'DRAFTS' ||
                   isDraftEmail(email);

    if (!isDraft) {
      // 普通邮件，直接返回原内容
      return email.body_html || email.body_text || '';
    }

    // 草稿邮件，只显示用户输入的内容，不显示原始邮件部分
    let content = email.body_text || email.body_html || '';

    if (!content) {
      return '[草稿无内容]';
    }

    // 提取用户输入的回复内容（在原始邮件分隔符之前的内容）
    const separators = [
      '---------- 原始邮件 ----------',  // 我们的新草稿格式
      '---------- 转发邮件 ----------',  // 我们的转发格式
      '\n\nFrom: ',                    // 标准邮件回复格式
      '-----原始邮件-----',             // 中文格式
      '\n\nOn ',                       // Gmail格式
      '\n\n> ',                        // 引用格式
    ];

    for (const separator of separators) {
      const index = content.indexOf(separator);
      if (index !== -1) {
        content = content.substring(0, index).trim();
        break;
      }
    }

    return content || '[草稿无内容]';
  }

  // 获取邮件预览文本（不包含引用信息，只显示用户回复内容）
  function getEmailPreviewWithoutQuote(email: EmailMessage, maxLength: number = 150): string {
    let content = '';

    // 优先使用纯文本内容
    if (email.body_text && email.body_text.trim()) {
      content = email.body_text.trim();
    } else if (email.body_html && email.body_html.trim()) {
      // 从HTML中提取纯文本
      content = simpleHtmlToText(email.body_html.trim());
    }

    if (!content) {
      return '[无内容]';
    }

    // 移除引用信息，只保留用户回复内容
    const separators = [
      '---------- 转发邮件 ----------',  // 我们的转发格式
      '---------------原始邮件---------------',
      '-----原始邮件-----',
      '-------- Original Message --------',
      '\n\nFrom: ',
      '\n\nOn ',
      '\n\n> ',
    ];

    for (const separator of separators) {
      const index = content.indexOf(separator);
      if (index !== -1) {
        content = content.substring(0, index).trim();
        break;
      }
    }

    // 清理内容
    content = cleanTextContent(content);

    if (!content || content.length === 0) {
      return '[无内容]';
    }

    return content.length > maxLength ? content.substring(0, maxLength) + '...' : content;
  }

  // 获取邮件内容预览文本（统一的预览逻辑）
  function getEmailPreviewText(email: EmailMessage, maxLength: number = 150): string {
    // 调试信息
    console.log('邮件预览调试:', {
      subject: email.subject,
      hasBodyText: !!email.body_text,
      hasBodyHtml: !!email.body_html,
      bodyTextLength: email.body_text?.length || 0,
      bodyHtmlLength: email.body_html?.length || 0,
      bodyTextPreview: email.body_text?.substring(0, 100),
      bodyHtmlPreview: email.body_html?.substring(0, 100)
    });

    // 特殊处理草稿邮件（包括在垃圾箱中的草稿邮件）
    // 检查是否是草稿邮件：当前在草稿文件夹，或者邮件内容包含草稿特征
    const isDraft = email.folder.toLowerCase() === 'drafts' ||
                   email.folder.toLowerCase() === '草稿' ||
                   email.folder.toUpperCase() === 'DRAFTS' ||
                   isDraftEmail(email);

    if (isDraft) {
      return getDraftPreviewText(email, maxLength);
    }

    // 优先使用纯文本内容，但也要清理CSS样式代码
    if (email.body_text && email.body_text.trim()) {
      const originalText = email.body_text.trim();
      const text = cleanTextContent(originalText);
      console.log('纯文本清理结果:', { original: originalText.substring(0, 100), cleaned: text.substring(0, 100) });
      if (text && text.length > 0) {
        return text.length > maxLength ? text.substring(0, maxLength) + '...' : text;
      }
    }

    // 如果没有纯文本，从HTML中提取
    if (email.body_html && email.body_html.trim()) {
      const originalHtml = email.body_html.trim();
      const htmlText = simpleHtmlToText(originalHtml);
      console.log('HTML清理结果:', { original: originalHtml.substring(0, 100), cleaned: htmlText.substring(0, 100) });
      if (htmlText && htmlText.length > 0) {
        return htmlText.length > maxLength ? htmlText.substring(0, maxLength) + '...' : htmlText;
      }
    }

    // 如果都没有内容，显示明确的提示
    console.log('邮件无内容:', email.subject);
    return '[此邮件无任何内容]';
  }

  // 获取草稿邮件的预览文本（只显示用户输入的内容）
  function getDraftPreviewText(email: EmailMessage, maxLength: number = 150): string {
    let content = '';

    // 优先使用纯文本内容
    if (email.body_text && email.body_text.trim()) {
      content = email.body_text.trim();
    } else if (email.body_html && email.body_html.trim()) {
      // 从HTML中提取纯文本
      content = email.body_html
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
    }

    if (!content) {
      return '[草稿无内容]';
    }



    // 提取用户输入的回复内容（在原始邮件分隔符之前的内容）
    const separators = [
      '---------- 原始邮件 ----------',  // 我们的新草稿格式
      '---------- 转发邮件 ----------',  // 我们的转发格式
      '\n\n---------- 原始邮件 ----------', // 带换行的格式
      'From:',                         // 标准邮件回复格式
      '-----原始邮件-----',             // 中文格式
      'On ',                           // Gmail格式
      '> ',                            // 引用格式
    ];

    let userContent = content;
    for (const separator of separators) {
      const separatorIndex = content.indexOf(separator);
      if (separatorIndex !== -1) {
        userContent = content.substring(0, separatorIndex).trim();

        break;
      }
    }

    // 如果没有找到分隔符，但内容很长，可能整个都是用户内容
    if (userContent === content && content.length > 500) {
      // 检查是否包含典型的原始邮件标识
      const originalEmailPatterns = [
        /发件人:\s*[^\n]+/,
        /发送时间:\s*[^\n]+/,
        /收件人:\s*[^\n]+/,
        /主题:\s*[^\n]+/,
        /From:\s*[^\n]+/,
        /Sent:\s*[^\n]+/,
        /To:\s*[^\n]+/,
        /Subject:\s*[^\n]+/
      ];

      for (const pattern of originalEmailPatterns) {
        const match = content.match(pattern);
        if (match) {
          const patternIndex = content.indexOf(match[0]);
          if (patternIndex > 0) {
            userContent = content.substring(0, patternIndex).trim();
            break;
          }
        }
      }
    }

    // 清理内容
    userContent = cleanTextContent(userContent);

    if (!userContent || userContent.length === 0) {
      return '[草稿无内容]';
    }

    return userContent.length > maxLength ? userContent.substring(0, maxLength) + '...' : userContent;
  }

  // 清理纯文本内容中的CSS样式代码和HTML残留
  function cleanTextContent(text: string): string {
    if (!text || !text.trim()) {
      return '';
    }

    let cleanedText = text;

    // 1. 移除HTML标签残留
    cleanedText = cleanedText.replace(/<[^>]*>/g, '');
    // 清理不完整的HTML标签
    cleanedText = cleanedText.replace(/<[^<]*$/g, '');
    cleanedText = cleanedText.replace(/^[^>]*>/g, '');

    // 2. 强力清理CSS样式文本和字体声明
    // 清理字体族声明，如 'Segoe UI Semibold';'Segoe UI';
    cleanedText = cleanedText.replace(/['"][^'"]*['"];?/g, ' ');
    // 清理CSS选择器和规则
    cleanedText = cleanedText.replace(/\s*[A-Za-z][A-Za-z0-9]*\s*\{[^}]*\}\s*/g, ' ');
    // 清理CSS属性
    cleanedText = cleanedText.replace(/\s*[a-zA-Z-]+\s*:\s*[^;{}]+;?\s*/g, ' ');
    // 清理花括号内容
    cleanedText = cleanedText.replace(/\s*\{[^}]*\}\s*/g, ' ');
    // 清理分号分隔的样式声明
    cleanedText = cleanedText.replace(/[^;]*;[^;]*;/g, ' ');

    // 3. 清理特定的CSS模式
    // 清理CSS单位
    cleanedText = cleanedText.replace(/\d+(\.\d+)?(px|em|rem|%|pt|pc|in|cm|mm|ex|ch|vw|vh|vmin|vmax);?/gi, ' ');
    // 清理颜色值
    cleanedText = cleanedText.replace(/#[0-9a-fA-F]{3,6};?/g, ' ');
    cleanedText = cleanedText.replace(/rgb\([^)]+\);?/gi, ' ');
    cleanedText = cleanedText.replace(/rgba\([^)]+\);?/gi, ' ');

    // 4. 移除特殊字符和符号
    cleanedText = cleanedText.replace(/[{}();:]/g, ' ');

    // 5. 清理多余的空白字符
    cleanedText = cleanedText
      .replace(/\s+/g, ' ')  // 多个空格合并为一个
      .replace(/\n\s+/g, '\n')  // 移除行首空格
      .replace(/\s+\n/g, '\n')  // 移除行尾空格
      .replace(/\n{3,}/g, '\n\n')  // 多个换行合并为两个
      .trim();

    // 6. 如果清理后内容为空或只包含空白字符，返回空字符串
    // 放宽条件：不再限制最小长度，允许短内容和包含数字/符号的内容
    if (!cleanedText || /^\s*$/.test(cleanedText)) {
      return '';
    }

    return cleanedText;
  }

  // 简单的HTML转文本函数 - 保留文本内容
  function simpleHtmlToText(html: string): string {
    if (!html || !html.trim()) {
      return '';
    }

    let text = html;

    // 1. 移除CSS样式块和脚本
    text = text.replace(/<style[^>]*>[\s\S]*?<\/style>/gi, '');
    text = text.replace(/<script[^>]*>[\s\S]*?<\/script>/gi, '');

    // 2. 处理换行标签（在移除标签之前）
    text = text
      .replace(/<br\s*\/?>/gi, '\n')
      .replace(/<\/p>/gi, '\n')
      .replace(/<\/div>/gi, '\n')
      .replace(/<\/h[1-6]>/gi, '\n')
      .replace(/<\/li>/gi, '\n')
      .replace(/<\/tr>/gi, '\n')
      .replace(/<\/td>/gi, ' ')
      .replace(/<\/th>/gi, ' ');

    // 3. 移除所有HTML标签
    text = text.replace(/<[^>]*>/g, '');

    // 4. 解码HTML实体
    text = text
      .replace(/&nbsp;/g, ' ')
      .replace(/&amp;/g, '&')
      .replace(/&lt;/g, '<')
      .replace(/&gt;/g, '>')
      .replace(/&quot;/g, '"')
      .replace(/&#39;/g, "'")
      .replace(/&apos;/g, "'");

    // 5. 清理多余的空白字符
    text = text
      .replace(/\s+/g, ' ')  // 多个空格合并为一个
      .replace(/\n\s+/g, '\n')  // 移除行首空格
      .replace(/\s+\n/g, '\n')  // 移除行尾空格
      .replace(/\n{3,}/g, '\n\n')  // 多个换行合并为两个
      .trim();

    return text;
  }

  // 改进的HTML清理函数 - 彻底清理HTML和CSS代码
  function cleanHtmlToText(html: string): string {
    if (!html || !html.trim()) {
      return '';
    }

    let text = html;

    // 1. 移除CSS样式块和脚本
    text = text.replace(/<style[^>]*>[\s\S]*?<\/style>/gi, '');
    text = text.replace(/<script[^>]*>[\s\S]*?<\/script>/gi, '');

    // 2. 处理换行标签（在移除标签之前）
    text = text
      .replace(/<br\s*\/?>/gi, '\n')
      .replace(/<\/p>/gi, '\n\n')
      .replace(/<\/div>/gi, '\n')
      .replace(/<\/h[1-6]>/gi, '\n\n')
      .replace(/<\/li>/gi, '\n')
      .replace(/<\/tr>/gi, '\n')
      .replace(/<\/td>/gi, ' ')
      .replace(/<\/th>/gi, ' ');

    // 3. 移除所有HTML标签（包括不完整的标签）
    text = text.replace(/<[^>]*>/g, '');
    // 清理残留的不完整标签（如 <td 没有闭合的）
    text = text.replace(/<[^<]*$/g, '');
    text = text.replace(/^[^>]*>/g, '');

    // 4. 强力清理CSS样式文本和字体声明
    // 清理字体族声明，如 'Segoe UI Semibold';'Segoe UI';'H...
    text = text.replace(/['"][^'"]*['"];?/g, ' ');
    // 清理CSS选择器和规则
    text = text.replace(/\s*[A-Za-z][A-Za-z0-9]*\s*\{[^}]*\}\s*/g, ' ');
    // 清理CSS属性
    text = text.replace(/\s*[a-zA-Z-]+\s*:\s*[^;{}]+;?\s*/g, ' ');
    // 清理花括号内容
    text = text.replace(/\s*\{[^}]*\}\s*/g, ' ');
    // 清理分号分隔的样式声明
    text = text.replace(/[^;]*;[^;]*;/g, ' ');

    // 5. 清理特定的CSS和样式模式
    // 清理字体相关的样式声明
    text = text.replace(/font-family\s*:\s*[^;]+;?/gi, ' ');
    text = text.replace(/font-size\s*:\s*[^;]+;?/gi, ' ');
    text = text.replace(/font-weight\s*:\s*[^;]+;?/gi, ' ');
    // 清理颜色和尺寸声明
    text = text.replace(/color\s*:\s*[^;]+;?/gi, ' ');
    text = text.replace(/background\s*:\s*[^;]+;?/gi, ' ');
    text = text.replace(/margin\s*:\s*[^;]+;?/gi, ' ');
    text = text.replace(/padding\s*:\s*[^;]+;?/gi, ' ');

    // 6. 清理残留的样式片段
    // 清理类似 'Segoe UI' 这样的字体名称
    text = text.replace(/['"][\w\s]+['"];?/g, ' ');
    // 清理CSS单位（px, em, rem, %, pt等）
    text = text.replace(/\d+(\.\d+)?(px|em|rem|%|pt|pc|in|cm|mm|ex|ch|vw|vh|vmin|vmax);?/gi, ' ');

    // 7. 解码HTML实体
    text = text
      .replace(/&nbsp;/g, ' ')
      .replace(/&amp;/g, '&')
      .replace(/&lt;/g, '<')
      .replace(/&gt;/g, '>')
      .replace(/&quot;/g, '"')
      .replace(/&#39;/g, "'")
      .replace(/&apos;/g, "'")
      .replace(/&copy;/g, '©')
      .replace(/&reg;/g, '®')
      .replace(/&trade;/g, '™');

    // 8. 最终清理：移除所有非文本字符和多余空白
    // 移除特殊字符和符号
    text = text.replace(/[{}();:]/g, ' ');
    // 清理多余的空白字符
    text = text
      .replace(/\s+/g, ' ')  // 多个空格合并为一个
      .replace(/\n\s+/g, '\n')  // 移除行首空格
      .replace(/\s+\n/g, '\n')  // 移除行尾空格
      .replace(/\n{3,}/g, '\n\n')  // 多个换行合并为两个
      .trim();

    // 9. 如果清理后内容为空或只包含空白字符，返回空字符串
    // 放宽条件：不再限制最小长度，允许短内容和包含数字/符号的内容
    if (!text || /^\s*$/.test(text)) {
      return '';
    }

    return text;
  }

  // 最简单的HTML清理函数，完全保持原始样式
  function sanitizeHtml(html: string): string {
    if (!html) return '';

    let cleanHtml = html;

    // 只移除危险的标签和属性，完全保持样式
    cleanHtml = cleanHtml
      .replace(/<script[^>]*>[\s\S]*?<\/script>/gi, '')  // 移除script标签
      .replace(/<iframe[^>]*>[\s\S]*?<\/iframe>/gi, '')  // 移除iframe标签
      .replace(/on\w+\s*=\s*["'][^"']*["']/gi, '')  // 移除事件处理器
      .replace(/javascript:/gi, '');  // 移除javascript协议

    // 为链接添加安全属性
    cleanHtml = cleanHtml.replace(/<a([^>]*)>/gi, (match, attrs) => {
      if (!attrs.includes('target=')) {
        return `<a${attrs} target="_blank" rel="noopener noreferrer">`;
      }
      return match;
    });

    return cleanHtml;
  }

  // 确保邮件内容正确显示
  function adjustEmailContentScale() {
    // 简化处理，主要通过CSS来实现响应式
    // 这个函数保留用于未来可能的优化
  }












  // Windows风格窗口控制函数
  async function minimizeWindow() {
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      const appWindow = getCurrentWindow();
      appWindow.minimize();
    } catch (error) {
      console.error('Failed to minimize window:', error);
    }
  }

  async function maximizeWindow() {
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      const appWindow = getCurrentWindow();
      appWindow.toggleMaximize();
      isWindowMaximized = !isWindowMaximized;
    } catch (error) {
      console.error('Failed to maximize window:', error);
    }
  }

  async function closeWindow() {
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      const appWindow = getCurrentWindow();
      appWindow.close();
    } catch (error) {
      console.error('Failed to close window:', error);
    }
  }



  // 移除 updateWindowState 函数，避免频繁查询窗口状态导致重绘
  // async function updateWindowState() {
  //   try {
  //     const { getCurrentWindow } = await import('@tauri-apps/api/window');
  //     const window = getCurrentWindow();
  //     isWindowMaximized = await window.isMaximized();
  //   } catch (error) {
  //     console.error('Failed to get window state:', error);
  //   }
  // }
  // 实时邮件监听初始化
  async function initRealTimeListener() {
    try {
      // 监听新邮件事件
      newMessagesUnlisten = await listen('new-messages', (event) => {
        const data = event.payload as {
          account_id: string;
          account_name: string;
          count: number;
        };

        // 如果正在清空缓存，忽略实时监听的新邮件通知
        if (isClearingCache) {
          console.log('正在清空缓存，忽略实时监听的新邮件通知');
          return;
        }

        // 立即同时触发所有通知（软件内、桌面、铃声）
        const notificationTitle = '新邮件通知';
        const notificationBody = `${data.account_name} 收到 ${data.count} 封新邮件`;

        console.log(`📧 [${new Date().toLocaleTimeString()}] 收到新邮件，立即触发所有通知`);

        // 1. 软件内通知（立即显示）
        addNotification('success', notificationBody);

        // 2. 桌面通知（立即触发，不等待）
        showDesktopNotification(notificationTitle, notificationBody, data.account_name);

        // 3. 铃声播放（立即触发，不等待）
        playNotificationSound();

        // 如果是当前账户，刷新邮件列表（不强制刷新，使用缓存优先）
        // 注意：实时监听已经保存了新邮件到数据库，这里只需要重新读取缓存即可
        if ($currentAccount && $currentAccount.id === data.account_id) {
          loadMessages($currentAccount.id, currentFolder.toUpperCase(), false);
        }
      });

      console.log('实时邮件监听初始化完成');
    } catch (error) {
      console.error('初始化实时邮件监听失败:', error);
    }
  }



  // 组件生命周期
  onMount(async () => {
    // 清理头像缓存，确保新的匹配逻辑生效
    clearAvatarCache();

    // 预加载通知API以减少延迟
    preloadNotificationAPI();

    // 加载保存的通知设置
    try {
      const savedSettings = localStorage.getItem('notificationSettings');
      if (savedSettings) {
        notificationSettings = { ...notificationSettings, ...JSON.parse(savedSettings) };
      }
    } catch (error) {
      console.warn('加载通知设置失败:', error);
    }

    // 加载保存的主题设置
    try {
      const savedTheme = localStorage.getItem('selectedTheme') as keyof typeof themes | 'custom';
      const savedCustomTheme = localStorage.getItem('customTheme');

      // 加载自定义主题配置
      if (savedCustomTheme) {
        try {
          customTheme = JSON.parse(savedCustomTheme);
        } catch (e) {
          console.warn('加载自定义主题失败:', e);
        }
      }

      // 设置当前主题
      if (savedTheme === 'custom') {
        currentTheme = 'custom';
      } else if (savedTheme && themes[savedTheme as keyof typeof themes]) {
        currentTheme = savedTheme as keyof typeof themes;
      }

      updateThemeColors();
    } catch (error) {
      console.warn('加载主题设置失败:', error);
    }

    // 如果启用了桌面通知，请求权限
    if (notificationSettings.enableDesktopNotification) {
      requestNotificationPermission();
    }

    initRealTimeListener();

    // 启动实时邮件监听
    try {
      await EmailAPI.startAutoSync();
      console.log('实时邮件监听已启动');
    } catch (error) {
      console.error('启动实时邮件监听失败:', error);
    }

    // 加载邮箱提供商列表
    try {
      providers = await EmailAPI.getEmailProviders();
      console.log('加载的邮箱提供商:', providers);
    } catch (error) {
      console.error('加载邮箱提供商失败:', error);
    }

    // 简化窗口状态初始化，避免频繁查询导致重绘
    isWindowMaximized = false; // 默认为非最大化状态

    // 设置链接拦截，让链接在外部浏览器打开
    setupLinkInterception();

    // Windows圆角问题的额外优化
    if (typeof window !== 'undefined') {
      // 减少页面重绘频率
      document.body.style.willChange = 'auto';

      // 优化滚动性能，减少重绘
      const containers = document.querySelectorAll('.email-cards-container, .accounts-sidebar');
      containers.forEach(container => {
        if (container instanceof HTMLElement) {
          container.style.willChange = 'scroll-position';
        }
      });
    }

    // 注释掉窗口状态监听器，避免频繁重绘导致圆角闪烁
    /*
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      const window = getCurrentWindow();

      let resizeTimeout: number;
      await window.listen('tauri://resize', async () => {
        if (resizeTimeout) {
          clearTimeout(resizeTimeout);
        }
        resizeTimeout = setTimeout(async () => {
          await updateWindowState();
          adjustEmailContentScale();
        }, 150);
      });
    } catch (error) {
      console.error('设置窗口状态监听器失败:', error);
    }
    */
  });

  onDestroy(() => {
    if (newMessagesUnlisten) {
      newMessagesUnlisten();
    }

    // 清理文件夹切换定时器
    if (folderSwitchTimeout) {
      clearTimeout(folderSwitchTimeout);
      folderSwitchTimeout = null;
    }

    // 清理加载状态
    loadingStates.clear();
  });
</script>

<!-- svelte-ignore a11y-click-events-have-key-events -->
<!-- svelte-ignore a11y-no-static-element-interactions -->
<div
  class="innovative-interface"
  on:click={(e) => {
    // 如果有选中的邮件，检查点击的元素
    if (selectedEmail) {
      // 如果正在翻译，不要关闭邮件详情
      if (isTranslating || (translatedEmails[selectedEmail.id]?.isTranslating)) {
        return;
      }

      const clickedElement = e.target as HTMLElement;
      const emailDetailSidebar = clickedElement.closest('.email-detail-sidebar');
      const emailCard = clickedElement.closest('.email-card');

      // 如果点击的不是邮件详情侧边栏，也不是邮件卡片，则关闭邮件详情
      if (!emailDetailSidebar && !emailCard) {
        selectedEmail = null;
      }
    }
  }}
>
  <!-- 顶部工具栏 -->
  <header class="toolbar">
    <!-- macOS风格时的左侧按钮 -->
    {#if titleBarStyle === 'mac'}
      <div class="mac-controls-container">
        <div class="window-controls mac-style">
          <button class="window-control-btn mac-close" title="关闭" aria-label="关闭窗口" on:click={closeWindow}>
            <svg width="8" height="8" viewBox="0 0 8 8">
              <path d="M1,1 L7,7 M1,7 L7,1" stroke="currentColor" stroke-width="1.2" fill="none"/>
            </svg>
          </button>
          <button class="window-control-btn mac-minimize" title="最小化" aria-label="最小化窗口" on:click={minimizeWindow}>
            <svg width="8" height="8" viewBox="0 0 8 8">
              <path d="M1,4 L7,4" stroke="currentColor" stroke-width="1.2" fill="none"/>
            </svg>
          </button>
          <button class="window-control-btn mac-maximize" title="{isWindowMaximized ? '向下还原' : '最大化'}" aria-label="{isWindowMaximized ? '向下还原窗口' : '最大化窗口'}" on:click={maximizeWindow}>
            {#if isWindowMaximized}
              <svg width="8" height="8" viewBox="0 0 8 8">
                <path d="M1.5,1.5 L6.5,1.5 L6.5,6.5 L1.5,6.5 Z M0.5,0.5 L5.5,0.5 L5.5,5.5" stroke="currentColor" stroke-width="1" fill="none"/>
              </svg>
            {:else}
              <svg width="8" height="8" viewBox="0 0 8 8">
                <rect x="1" y="1" width="6" height="6" stroke="currentColor" stroke-width="1" fill="none"/>
              </svg>
            {/if}
          </button>
        </div>
      </div>
    {/if}

    <div class="toolbar-left">
      {#if titleBarStyle !== 'mac'}
        <div class="app-title" data-tauri-drag-region>
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z"></path>
            <polyline points="22,6 12,13 2,6"></polyline>
          </svg>
          <span>XMail Pro</span>
        </div>
      {/if}
      <!-- 专用拖拽区域 -->
      <div class="drag-region" data-tauri-drag-region></div>
    </div>

    <div class="toolbar-right">
      <!-- 主题切换器 -->
      <div class="theme-switcher">
        {#each Object.entries(themes) as [themeKey, theme]}
          <button
            class="theme-btn {currentTheme === themeKey ? 'active' : ''}"
            style="background: linear-gradient(135deg, {theme.primary}, {theme.secondary})"
            title={theme.name}
            aria-label="切换到{theme.name}主题"
            on:click={() => switchTheme(themeKey as keyof typeof themes)}
          >
          </button>
        {/each}

        <!-- 自定义主题按钮 -->
        <button
          class="theme-btn custom-theme-btn {currentTheme === 'custom' ? 'active' : ''}"
          style="background: linear-gradient(135deg, {customTheme.primary}, {customTheme.secondary})"
          title={customTheme.name}
          aria-label="切换到{customTheme.name}"
          on:click={() => switchTheme('custom')}
        >
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"></path>
            <path d="m18.5 2.5 3 3L12 15l-4 1 1-4 9.5-9.5z"></path>
          </svg>
        </button>
      </div>

      <!-- 搜索容器 -->
      <div class="search-container">
        <svg class="search-icon" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <circle cx="11" cy="11" r="8"></circle>
          <path d="m21 21-4.35-4.35"></path>
        </svg>
        <input
          type="text"
          placeholder="搜索邮件、联系人..."
          bind:value={searchQuery}
          class="search-input"
        />
        <button class="search-filter-btn" title="搜索筛选" aria-label="搜索筛选">
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polygon points="22,3 2,3 10,12.46 10,19 14,21 14,12.46"></polygon>
          </svg>
        </button>
      </div>

      <!-- 视图切换器 -->
      <div class="view-switcher">
        <div class="switcher-track">
          <div class="switcher-thumb" class:pos-0={viewMode === 'list'} class:pos-1={viewMode === 'timeline'} class:pos-2={viewMode === 'chat'}></div>
        </div>
        <button
          class="switch-btn {viewMode === 'list' ? 'active' : ''}"
          on:click={() => switchViewMode('list')}
          title="列表视图"
          aria-label="列表视图"
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <line x1="8" y1="6" x2="21" y2="6"></line>
            <line x1="8" y1="12" x2="21" y2="12"></line>
            <line x1="8" y1="18" x2="21" y2="18"></line>
            <circle cx="4" cy="6" r="1"></circle>
            <circle cx="4" cy="12" r="1"></circle>
            <circle cx="4" cy="18" r="1"></circle>
          </svg>
        </button>
        <button
          class="switch-btn {viewMode === 'timeline' ? 'active' : ''}"
          on:click={() => switchViewMode('timeline')}
          title="时间线视图"
          aria-label="时间线视图"
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <rect x="3" y="4" width="18" height="18" rx="2" ry="2"></rect>
            <line x1="16" y1="2" x2="16" y2="6"></line>
            <line x1="8" y1="2" x2="8" y2="6"></line>
            <line x1="3" y1="10" x2="21" y2="10"></line>
          </svg>
        </button>
        <button
          class="switch-btn {viewMode === 'chat' ? 'active' : ''}"
          on:click={() => switchViewMode('chat')}
          title="对话视图"
          aria-label="对话视图"
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"></path>
          </svg>
        </button>
      </div>

      <!-- 操作按钮 -->
      <div class="action-buttons">
        <button class="toolbar-btn refresh-btn" title="刷新邮件" aria-label="刷新邮件" on:click={refreshMessages} disabled={isLoadingMessages}>
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8"></path>
            <path d="M21 3v5h-5"></path>
            <path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16"></path>
            <path d="M3 21v-5h5"></path>
          </svg>
        </button>
        <button
          class="toolbar-btn realtime-btn {realTimeListenerEnabled ? 'active' : ''}"
          title="{realTimeListenerEnabled ? '实时监听已启用' : '实时监听未启用'}"
          aria-label="实时邮件监听状态"
          disabled
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polygon points="13,2 3,14 12,14 11,22 21,10 12,10"></polygon>
          </svg>
        </button>
        <button class="toolbar-btn" title="设置" aria-label="设置" on:click={openSettingsPage}>
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="12" cy="12" r="3"></circle>
            <path d="M19.4,15a1.65,1.65,0,0,0,.33,1.82l.06.06a2,2,0,0,1,0,2.83,2,2,0,0,1-2.83,0l-.06-.06a1.65,1.65,0,0,0-1.82-.33,1.65,1.65,0,0,0-1,1.51V21a2,2,0,0,1-2,2,2,2,0,0,1-2-2V20.51a1.65,1.65,0,0,0-1.08-1.51,1.65,1.65,0,0,0-1.82.33l-.06.06a2,2,0,0,1-2.83,0,2,2,0,0,1,0-2.83l.06-.06a1.65,1.65,0,0,0,.33-1.82,1.65,1.65,0,0,0-1.51-1H3a2,2,0,0,1-2-2,2,2,0,0,1,2-2H3.49a1.65,1.65,0,0,0,1.51-1.08,1.65,1.65,0,0,0-.33-1.82L4.61,8.07a2,2,0,0,1,0-2.83,2,2,0,0,1,2.83,0L7.5,5.3a1.65,1.65,0,0,0,1.82.33H9.32a1.65,1.65,0,0,0,1-1.51V3a2,2,0,0,1,2-2,2,2,0,0,1,2,2V3.49a1.65,1.65,0,0,0,1.08,1.51,1.65,1.65,0,0,0,1.82-.33l.06-.06a2,2,0,0,1,2.83,0,2,2,0,0,1,0,2.83L20.05,7.5a1.65,1.65,0,0,0-.33,1.82v0a1.65,1.65,0,0,0,1.51,1H21a2,2,0,0,1,2,2,2,2,0,0,1-2,2H20.51A1.65,1.65,0,0,0,19.4,15Z"></path>
          </svg>
        </button>
        <button class="toolbar-btn clear-cache-btn" on:click={clearEmailCache} title="清空缓存" aria-label="清空邮件缓存">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M3 6h18"></path>
            <path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"></path>
            <path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"></path>
            <line x1="10" y1="11" x2="10" y2="17"></line>
            <line x1="14" y1="11" x2="14" y2="17"></line>
          </svg>
        </button>

        <button class="toolbar-btn add-account-btn" on:click={addAccount} title="添加账户" aria-label="添加账户">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2"></path>
            <circle cx="9" cy="7" r="4"></circle>
            <line x1="19" y1="8" x2="19" y2="14"></line>
            <line x1="22" y1="11" x2="16" y2="11"></line>
          </svg>
        </button>
      </div>

      <!-- macOS风格时的右侧标题 -->
      {#if titleBarStyle === 'mac'}
        <div class="app-title mac-title-right" data-tauri-drag-region>
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z"></path>
            <polyline points="22,6 12,13 2,6"></polyline>
          </svg>
          <span>XMail Pro</span>
        </div>
      {:else}
        <!-- Windows风格窗口控制按钮 -->
        <div class="window-controls windows-style">
          <button class="window-control-btn minimize-btn" title="最小化" aria-label="最小化窗口" on:click={minimizeWindow}>
            <svg width="10" height="10" viewBox="0 0 10 10">
              <path d="M0,5 L10,5" stroke="currentColor" stroke-width="1" fill="none"/>
            </svg>
          </button>
          <button class="window-control-btn maximize-btn" title="{isWindowMaximized ? '向下还原' : '最大化'}" aria-label="{isWindowMaximized ? '向下还原窗口' : '最大化窗口'}" on:click={maximizeWindow}>
            {#if isWindowMaximized}
              <svg width="10" height="10" viewBox="0 0 10 10">
                <path d="M3,3 L9,3 L9,9 L3,9 Z" stroke="currentColor" stroke-width="1" fill="none"/>
                <path d="M1,1 L7,1 L7,7 L1,7 Z" stroke="currentColor" stroke-width="1" fill="none"/>
              </svg>
            {:else}
              <svg width="10" height="10" viewBox="0 0 10 10">
                <path d="M1,1 L9,1 L9,9 L1,9 Z" stroke="currentColor" stroke-width="1" fill="none"/>
              </svg>
            {/if}
          </button>
          <button class="window-control-btn close-btn" title="关闭" aria-label="关闭窗口" on:click={closeWindow}>
            <svg width="10" height="10" viewBox="0 0 10 10">
              <path d="M1,1 L9,9 M9,1 L1,9" stroke="currentColor" stroke-width="1" fill="none"/>
            </svg>
          </button>
        </div>
      {/if}
    </div>
  </header>

  <!-- 主布局容器 -->
  <div class="main-layout">
    <!-- 左侧账号列表 - 设置页面时隐藏 -->
    {#if !showSettingsPage}
    <aside class="accounts-sidebar">
      <div class="sidebar-header">
        <div class="sidebar-tabs">
          <button
            class="sidebar-tab {sidebarMode === 'accounts' ? 'active' : ''}"
            on:click={() => switchSidebarMode('accounts')}
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"></path>
              <circle cx="12" cy="7" r="4"></circle>
            </svg>
            账号
          </button>
          <button
            class="sidebar-tab {sidebarMode === 'contacts' ? 'active' : ''}"
            on:click={() => switchSidebarMode('contacts')}
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2"></path>
              <circle cx="9" cy="7" r="4"></circle>
              <path d="M23 21v-2a4 4 0 0 0-3-3.87"></path>
              <path d="M16 3.13a4 4 0 0 1 0 7.75"></path>
            </svg>
            联系人
          </button>
        </div>
      </div>
      <div class="accounts-list">
        {#if sidebarMode === 'accounts'}
          <!-- 邮箱账号模式 -->
          {#if $accounts.length === 0}
          <!-- 没有账户时显示邮箱提供商选项 -->
          <div class="no-accounts">
            <p class="no-accounts-title">选择邮箱提供商快速添加</p>
            <div class="provider-list">
              {#each providers as provider}
                <button
                  class="provider-item"
                  on:click={() => quickAddProvider(provider)}
                  aria-label="添加 {provider.name} 账户"
                >
                  <div class="provider-icon-wrapper">
                    <img src={getProviderIconPath(provider.name)} alt={provider.name} class="provider-icon-img" />
                  </div>
                  <span class="provider-name">{provider.name}</span>
                  <span class="add-icon">+</span>
                </button>
              {/each}
            </div>
            <div class="manual-add">
              <button class="manual-add-btn" on:click={addAccount}>
                ⚙️ 手动配置其他邮箱
              </button>
            </div>
          </div>
        {:else}
          <!-- 有账户时显示账户分组 -->
          {#each Object.entries(groupedAccounts) as [provider, accounts]}
            <div class="account-group">
              <button class="group-header" on:click={() => toggleGroup(provider)} aria-label="展开/折叠{provider}账号组">
                <div class="group-icon">
                  <img src={getProviderIconPath(provider)} alt="{provider}" class="provider-icon" />
                </div>
                <div class="group-info">
                  <div class="group-name">{getProviderDisplayName(provider)}</div>
                  <div class="group-summary">{accounts.length} 个账号</div>
                </div>
                <div class="group-toggle {expandedGroups[provider] ? 'expanded' : ''}">
                  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <polyline points="6,9 12,15 18,9"></polyline>
                  </svg>
                </div>
              </button>
              {#if expandedGroups[provider]}
                <div class="group-accounts">
                  {#each accounts as account}
                    <div class="account-item {$currentAccount?.id === account.id ? 'active' : ''}"
                         on:click={() => selectAccount(account)}
                         on:keydown={(e) => e.key === 'Enter' && selectAccount(account)}
                         role="button"
                         tabindex="0">
                      <div class="account-info">
                        <div class="account-name">{account.name}</div>
                        <div class="account-email">{account.email}</div>
                      </div>
                    </div>
                  {/each}
                </div>
              {/if}
            </div>
          {/each}
        {/if}
        {:else}
          <!-- 邮箱联系人模式 -->
          <div class="contacts-section">
            <div class="contacts-header">
              <h4>邮箱联系人</h4>
              <button class="add-contact-btn" title="添加联系人" aria-label="添加联系人" on:click={openAddContactDialog}>
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <line x1="12" y1="5" x2="12" y2="19"></line>
                  <line x1="5" y1="12" x2="19" y2="12"></line>
                </svg>
              </button>
            </div>
            <div class="contacts-list">
              {#if isLoadingContacts}
                <div class="loading-contacts">
                  <div class="loading-spinner"></div>
                  <p>加载联系人中...</p>
                </div>
              {:else if contacts.length === 0}
                <div class="no-contacts">
                  <div class="no-contacts-icon">
                    <img src="/lianxir.svg" alt="联系人图标" />
                  </div>
                  <p class="no-contacts-title">暂无联系人</p>
                  <p class="no-contacts-desc">点击上方 + 按钮添加常用邮箱联系人</p>
                </div>
              {:else}
                {#each contacts as contact (contact.id)}
                  <div class="contact-item">
                    <div class="contact-avatar">
                      <div class="contact-avatar-placeholder">
                        {contact.name.charAt(0).toUpperCase()}
                      </div>
                      {#if contact.is_favorite}
                        <div class="contact-favorite-badge">
                          <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor">
                            <path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z"/>
                          </svg>
                        </div>
                      {/if}
                    </div>
                    <div class="contact-info">
                      <div class="contact-name">{contact.name}</div>
                      <div class="contact-email">{contact.email}</div>
                      {#if contact.company}
                        <div class="contact-company">{contact.company}</div>
                      {/if}
                    </div>
                    <div class="contact-actions">
                      <button
                        class="contact-action-btn"
                        title="发送邮件"
                        aria-label="发送邮件给 {contact.name}"
                        on:click={() => openComposeEmailToContact(contact)}
                      >
                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                          <path d="M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z"/>
                          <polyline points="22,6 12,13 2,6"/>
                        </svg>
                      </button>
                    </div>
                  </div>
                {/each}
              {/if}
            </div>
          </div>
        {/if}
      </div>
    </aside>
    {/if}

    <!-- 右侧邮件内容区域 -->
    <main class="main-content-wrapper {showSettingsPage ? 'full-width' : ''}">
      <div class="main-content">
    {#if showSettingsPage}
      <!-- 设置页面 -->
      <div class="settings-page">
        <div class="settings-header">
          <h2>设置</h2>
          <button class="close-settings-btn" on:click={closeSettingsPage} title="关闭设置" aria-label="关闭设置">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <line x1="18" y1="6" x2="6" y2="18"></line>
              <line x1="6" y1="6" x2="18" y2="18"></line>
            </svg>
          </button>
        </div>

        <div class="settings-content">
          <!-- 左侧设置菜单 -->
          <aside class="settings-sidebar">
            <nav class="settings-nav">
              <button
                class="settings-nav-item {currentSettingsTab === 'personal' ? 'active' : ''}"
                on:click={() => switchSettingsTab('personal')}
              >
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"></path>
                  <circle cx="12" cy="7" r="4"></circle>
                </svg>
                个人账号
              </button>
              <button
                class="settings-nav-item {currentSettingsTab === 'email' ? 'active' : ''}"
                on:click={() => switchSettingsTab('email')}
              >
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z"></path>
                  <polyline points="22,6 12,13 2,6"></polyline>
                </svg>
                邮箱账号
              </button>
              <button
                class="settings-nav-item {currentSettingsTab === 'mail-notifications' ? 'active' : ''}"
                on:click={() => switchSettingsTab('mail-notifications')}
              >
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9"></path>
                  <path d="M13.73 21a2 2 0 0 1-3.46 0"></path>
                </svg>
                邮件通知
              </button>
              <button
                class="settings-nav-item {currentSettingsTab === 'theme' ? 'active' : ''}"
                on:click={() => switchSettingsTab('theme')}
              >
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <circle cx="12" cy="12" r="5"></circle>
                  <line x1="12" y1="1" x2="12" y2="3"></line>
                  <line x1="12" y1="21" x2="12" y2="23"></line>
                  <line x1="4.22" y1="4.22" x2="5.64" y2="5.64"></line>
                  <line x1="18.36" y1="18.36" x2="19.78" y2="19.78"></line>
                  <line x1="1" y1="12" x2="3" y2="12"></line>
                  <line x1="21" y1="12" x2="23" y2="12"></line>
                  <line x1="4.22" y1="19.78" x2="5.64" y2="18.36"></line>
                  <line x1="18.36" y1="5.64" x2="19.78" y2="4.22"></line>
                </svg>
                主题设置
              </button>
              <button
                class="settings-nav-item {currentSettingsTab === 'system' ? 'active' : ''}"
                on:click={() => switchSettingsTab('system')}
              >
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <circle cx="12" cy="12" r="3"></circle>
                  <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1 1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"></path>
                </svg>
                系统设置
              </button>
            </nav>
          </aside>

          <!-- 右侧设置内容 -->
          <div class="settings-main">
            {#if currentSettingsTab === 'personal'}
              <!-- 朦胧气泡背景 -->
              <div class="bubble-background">
                <div class="bubble bubble-1"></div>
                <div class="bubble bubble-2"></div>
                <div class="bubble bubble-3"></div>
                <div class="bubble bubble-4"></div>
                <div class="bubble bubble-5"></div>
                <div class="bubble bubble-6"></div>
              </div>

              <div class="settings-section">
                <h3>个人账号</h3>
                <p class="settings-description">查看和管理您的登录账号信息</p>

                <!-- 用户信息卡片 -->
                {#if $currentUser}
                  {console.log('设置页面 - 当前用户信息:', $currentUser)}
                  <div class="user-info-card">
                    <div class="user-avatar">
                      {#if $currentUser.avatar}
                        <div class="avatar-circle">
                          <img
                            src={$currentUser.avatar}
                            alt={$currentUser.username}
                            class="avatar-image"
                            on:load={() => console.log('头像加载成功:', $currentUser.avatar)}
                            on:error={(e) => {
                              // 头像加载失败，显示首字母
                              const target = e.target as HTMLImageElement;
                              const parent = target?.parentElement as HTMLElement;
                              if (target && parent) {
                                target.style.display = 'none';
                                parent.innerHTML = $currentUser.username.charAt(0).toUpperCase();
                                parent.style.background = 'linear-gradient(135deg, var(--theme-primary), var(--theme-secondary))';
                                parent.style.color = 'white';
                                parent.style.display = 'flex';
                                parent.style.alignItems = 'center';
                                parent.style.justifyContent = 'center';
                                parent.style.fontSize = '24px';
                                parent.style.fontWeight = '600';
                              }
                            }}
                          />
                        </div>
                      {:else}
                        <div class="avatar-circle">
                          {$currentUser.username.charAt(0).toUpperCase()}
                        </div>
                      {/if}
                    </div>
                    <div class="user-details">
                      <h4 class="user-name">{$currentUser.username}</h4>
                      <p class="user-email">
                        {#if $currentUser.authType === 'qq_oauth2'}
                          通过QQ登录
                        {:else}
                          {$currentUser.email}
                        {/if}
                      </p>
                      <div class="user-auth-type">
                        <span class="auth-badge {$currentUser.authType === 'qq_oauth2' ? 'qq' : ($currentUser.authType === 'oauth2' ? ($currentUser.email && $currentUser.email.includes('@gmail.com') ? 'google' : 'oauth2') : 'password')}">
                          {#if $currentUser.authType === 'qq_oauth2'}
                            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                              <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 15l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z"/>
                            </svg>
                            QQ 登录
                          {:else if $currentUser.authType === 'oauth2'}
                            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                              <path d="M9 12l2 2 4-4"></path>
                              <circle cx="12" cy="12" r="9"></circle>
                            </svg>
                            {#if $currentUser.email && $currentUser.email.includes('@gmail.com')}
                              Google 登录
                            {:else}
                              OAuth2 登录
                            {/if}
                          {:else}
                            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                              <rect x="3" y="11" width="18" height="11" rx="2" ry="2"></rect>
                              <circle cx="12" cy="16" r="1"></circle>
                              <path d="M7 11V7a5 5 0 0 1 10 0v4"></path>
                            </svg>
                            密码登录
                          {/if}
                        </span>
                      </div>
                    </div>
                  </div>

                  <!-- 账号操作 -->
                  <div class="setting-item">
                    <div class="setting-header">
                      <h4>账号操作</h4>
                    </div>
                    <div class="account-actions">
                      <button class="settings-btn danger" on:click={logout}>
                        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                          <path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"></path>
                          <polyline points="16,17 21,12 16,7"></polyline>
                          <line x1="21" y1="12" x2="9" y2="12"></line>
                        </svg>
                        退出登录
                      </button>
                    </div>
                  </div>
                {:else}
                  <div class="settings-placeholder">
                    <div class="placeholder-icon">👤</div>
                    <p>未登录用户</p>
                  </div>
                {/if}
              </div>
            {:else if currentSettingsTab === 'email'}
              <!-- 朦胧气泡背景 -->
              <div class="bubble-background">
                <div class="bubble bubble-1"></div>
                <div class="bubble bubble-2"></div>
                <div class="bubble bubble-3"></div>
                <div class="bubble bubble-4"></div>
                <div class="bubble bubble-5"></div>
                <div class="bubble bubble-6"></div>
              </div>

              <div class="settings-section">
                <h3>邮箱账号</h3>
                <p class="settings-description">管理您的邮箱账号和连接设置</p>

                <!-- 邮箱账号列表 -->
                {#if $accounts.length > 0}
                  <div class="email-accounts-list">
                    {#each $accounts as account}
                      <div class="email-account-card">
                        <div class="settings-account-info">
                          <div class="account-avatar">
                            <div class="settings-provider-icon">
                              <img src={getProviderIconPath(getEmailProvider(account.email))} alt="邮箱提供商" class="provider-icon-img" />
                            </div>
                          </div>
                          <div class="account-details">
                            <h4 class="settings-account-name">{account.name}</h4>
                            <p class="settings-account-email">{account.email}</p>
                            <div class="account-status">
                              <span class="status-badge {account.auth_type === 'oauth2' ? 'oauth2' : 'password'}">
                                {account.auth_type === 'oauth2' ? 'OAuth2' : '密码认证'}
                              </span>
                            </div>
                          </div>
                        </div>
                        <div class="account-actions">

                          <button class="settings-btn small danger" on:click={() => deleteEmailAccount(account)}>
                            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                              <polyline points="3,6 5,6 21,6"></polyline>
                              <path d="m19,6v14a2,2 0 0,1-2,2H7a2,2 0 0,1-2-2V6m3,0V4a2,2 0 0,1 2-2h4a2,2 0 0,1 2,2v2"></path>
                            </svg>
                            删除
                          </button>
                        </div>
                      </div>
                    {/each}
                  </div>

                  <!-- 添加新账号按钮 -->
                  <div class="add-account-section">
                    <button class="settings-btn primary" on:click={() => showAccountDialog.set(true)}>
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                        <line x1="12" y1="5" x2="12" y2="19"></line>
                        <line x1="5" y1="12" x2="19" y2="12"></line>
                      </svg>
                      添加邮箱账号
                    </button>
                  </div>
                {:else}
                  <div class="settings-placeholder">
                    <div class="placeholder-icon">📧</div>
                    <p>暂无邮箱账号</p>
                    <button class="settings-btn primary" on:click={() => showAccountDialog.set(true)}>
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                        <line x1="12" y1="5" x2="12" y2="19"></line>
                        <line x1="5" y1="12" x2="19" y2="12"></line>
                      </svg>
                      添加第一个邮箱账号
                    </button>
                  </div>
                {/if}
              </div>
            {:else if currentSettingsTab === 'mail-notifications'}
              <!-- 朦胧气泡背景 -->
              <div class="bubble-background">
                <div class="bubble bubble-1"></div>
                <div class="bubble bubble-2"></div>
                <div class="bubble bubble-3"></div>
                <div class="bubble bubble-4"></div>
                <div class="bubble bubble-5"></div>
                <div class="bubble bubble-6"></div>
              </div>

              <div class="settings-section">
                <h3>邮件通知</h3>
                <p class="settings-description">配置邮件通知和提醒设置</p>

                <!-- 桌面通知设置 -->
                <div class="setting-item">
                  <div class="setting-header">
                    <h4>桌面通知</h4>
                    <label class="toggle-switch">
                      <input
                        type="checkbox"
                        bind:checked={notificationSettings.enableDesktopNotification}
                        on:change={() => updateNotificationSettings('enableDesktopNotification', notificationSettings.enableDesktopNotification)}
                      />
                      <span class="toggle-slider"></span>
                    </label>
                  </div>
                  <p class="setting-description">收到新邮件时显示桌面通知</p>
                </div>

                <!-- 声音通知设置 -->
                <div class="setting-item">
                  <div class="setting-header">
                    <h4>声音通知</h4>
                    <label class="toggle-switch">
                      <input
                        type="checkbox"
                        bind:checked={notificationSettings.enableSound}
                        on:change={() => updateNotificationSettings('enableSound', notificationSettings.enableSound)}
                      />
                      <span class="toggle-slider"></span>
                    </label>
                  </div>
                  <p class="setting-description">收到新邮件时播放提示音</p>
                </div>

                <!-- 铃声选择 -->
                {#if notificationSettings.enableSound}
                  <div class="setting-item">
                    <div class="setting-header">
                      <h4>选择铃声</h4>
                    </div>
                    <p class="setting-description">选择您喜欢的新邮件提示音</p>

                    <div class="sound-options">
                      <div class="sound-option">
                        <label class="sound-radio">
                          <input
                            type="radio"
                            name="notification-sound"
                            value="lingsheng1"
                            bind:group={notificationSettings.selectedSound}
                            on:change={() => updateNotificationSettings('selectedSound', notificationSettings.selectedSound)}
                          />
                          <span class="radio-mark"></span>
                          <span class="sound-name">铃声 1</span>
                        </label>
                        <button
                          class="test-sound-btn"
                          on:click={() => testPlaySound('lingsheng1')}
                          title="试听铃声1"
                          aria-label="试听铃声1"
                        >
                          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                            <polygon points="5,3 19,12 5,21"></polygon>
                          </svg>
                        </button>
                      </div>

                      <div class="sound-option">
                        <label class="sound-radio">
                          <input
                            type="radio"
                            name="notification-sound"
                            value="lingsheng2"
                            bind:group={notificationSettings.selectedSound}
                            on:change={() => updateNotificationSettings('selectedSound', notificationSettings.selectedSound)}
                          />
                          <span class="radio-mark"></span>
                          <span class="sound-name">铃声 2</span>
                        </label>
                        <button
                          class="test-sound-btn"
                          on:click={() => testPlaySound('lingsheng2')}
                          title="试听铃声2"
                          aria-label="试听铃声2"
                        >
                          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                            <polygon points="5,3 19,12 5,21"></polygon>
                          </svg>
                        </button>
                      </div>
                    </div>
                  </div>
                {/if}
              </div>
            {:else if currentSettingsTab === 'theme'}
              <!-- 朦胧气泡背景 -->
              <div class="bubble-background">
                <div class="bubble bubble-1"></div>
                <div class="bubble bubble-2"></div>
                <div class="bubble bubble-3"></div>
                <div class="bubble bubble-4"></div>
                <div class="bubble bubble-5"></div>
                <div class="bubble bubble-6"></div>
              </div>

              <div class="settings-section">
                <h3>主题设置</h3>
                <p class="settings-description">个性化您的应用外观和主题</p>

                <!-- 主题选择 -->
                <div class="setting-item">
                  <div class="setting-header">
                    <h4>选择主题</h4>
                  </div>
                  <p class="setting-description">选择您喜欢的应用主题色彩</p>

                  <div class="theme-options">
                    {#each Object.entries(themes) as [themeKey, theme]}
                      <button
                        class="theme-option {currentTheme === themeKey ? 'active' : ''}"
                        on:click={() => switchTheme(themeKey as keyof typeof themes)}
                      >
                        <div class="theme-preview">
                          <div class="theme-color primary" style="background: {theme.primary}"></div>
                          <div class="theme-color secondary" style="background: {theme.secondary}"></div>
                        </div>
                        <span class="theme-name">{theme.name}</span>
                      </button>
                    {/each}

                    <!-- 自定义主题选项 -->
                    <div
                      class="theme-option custom-theme-option {currentTheme === 'custom' ? 'active' : ''}"
                      role="button"
                      tabindex="0"
                      on:click={() => switchTheme('custom')}
                      on:keydown={(e) => e.key === 'Enter' && switchTheme('custom')}
                    >
                      <div class="theme-preview">
                        <div class="theme-color primary" style="background: {customTheme.primary}"></div>
                        <div class="theme-color secondary" style="background: {customTheme.secondary}"></div>
                      </div>
                      <span class="theme-name">{customTheme.name}</span>
                      <div class="custom-theme-actions">
                        <button
                          class="edit-custom-theme-btn"
                          on:click|stopPropagation={() => openCustomThemeDialog()}
                          title="编辑自定义主题"
                          aria-label="编辑自定义主题"
                        >
                          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                            <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"></path>
                            <path d="m18.5 2.5 3 3L12 15l-4 1 1-4 9.5-9.5z"></path>
                          </svg>
                        </button>
                      </div>
                    </div>
                  </div>
                </div>
              </div>
            {:else if currentSettingsTab === 'system'}
              <!-- 朦胧气泡背景 -->
              <div class="bubble-background">
                <div class="bubble bubble-1"></div>
                <div class="bubble bubble-2"></div>
                <div class="bubble bubble-3"></div>
                <div class="bubble bubble-4"></div>
                <div class="bubble bubble-5"></div>
                <div class="bubble bubble-6"></div>
              </div>

              <div class="settings-section">
                <h3>系统设置</h3>
                <p class="settings-description">配置应用的系统级设置</p>

                <!-- 标题栏样式设置 -->
                <div class="setting-item">
                  <div class="setting-header">
                    <h4>标题栏样式</h4>
                  </div>
                  <p class="setting-description">选择窗口标题栏的控制按钮样式</p>

                  <div class="titlebar-style-section">
                    <div class="style-options">
                      <label class="style-option" class:active={titleBarStyle === 'windows'}>
                        <input type="radio" bind:group={titleBarStyle} value="windows" />
                        <div class="option-content">
                          <div class="option-preview windows-preview titlebar-preview">
                            <div class="preview-titlebar windows-layout">
                              <div class="preview-title windows-title-left">
                                <svg class="preview-icon" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                  <path d="M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z"></path>
                                  <polyline points="22,6 12,13 2,6"></polyline>
                                </svg>
                                <span>Xmail Pro</span>
                              </div>
                              <div class="preview-controls windows-controls">
                                <div class="control-btn minimize">−</div>
                                <div class="control-btn maximize">□</div>
                                <div class="control-btn close">×</div>
                              </div>
                            </div>
                          </div>
                          <div class="option-info">
                            <div class="option-title">Windows 风格</div>
                            <div class="option-desc">控制按钮位于右上角</div>
                          </div>
                        </div>
                      </label>

                      <label class="style-option" class:active={titleBarStyle === 'mac'}>
                        <input type="radio" bind:group={titleBarStyle} value="mac" />
                        <div class="option-content">
                          <div class="option-preview mac-preview titlebar-preview">
                            <div class="preview-titlebar mac-layout">
                              <div class="preview-controls mac-controls">
                                <div class="control-btn close mac-close"></div>
                                <div class="control-btn minimize mac-minimize"></div>
                                <div class="control-btn maximize mac-maximize"></div>
                              </div>
                              <div class="preview-title mac-title-right">
                                <svg class="preview-icon" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                  <path d="M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z"></path>
                                  <polyline points="22,6 12,13 2,6"></polyline>
                                </svg>
                                <span>Xmail Pro</span>
                              </div>
                            </div>
                          </div>
                          <div class="option-info">
                            <div class="option-title">macOS 风格</div>
                            <div class="option-desc">控制按钮位于左上角</div>
                          </div>
                        </div>
                      </label>
                    </div>

                    <div class="titlebar-info">
                      <div class="info-item">
                        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                          <circle cx="12" cy="12" r="10"></circle>
                          <line x1="12" y1="16" x2="12" y2="12"></line>
                          <circle cx="12" cy="8" r="1" fill="currentColor"></circle>
                        </svg>
                        <span>更改标题栏样式会立即生效</span>
                      </div>
                    </div>
                  </div>
                </div>

                <!-- 数据库设置 -->
                <div class="setting-item">
                  <div class="setting-header">
                    <h4>数据存储位置</h4>
                  </div>
                  <p class="setting-description">自定义SQLite邮件数据库的存储位置</p>

                  <div class="database-path-section">
                    <div class="current-path">
                      <div class="path-label">当前数据库路径：</div>
                      <div class="path-display">
                        <span class="path-text">{currentDatabasePath || '获取中...'}</span>
                        <button class="copy-path-btn" on:click={() => navigator.clipboard.writeText(currentDatabasePath)} title="复制路径" aria-label="复制数据库路径">
                          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                            <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
                            <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
                          </svg>
                        </button>
                      </div>
                    </div>

                    <div class="database-actions">
                      <button class="settings-btn secondary" on:click={() => showDatabasePathDialog = true}>
                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                          <path d="M3 7v10a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V9a2 2 0 0 0-2-2H5a2 2 0 0 0-2-2z"></path>
                          <path d="M8 21v-4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v4"></path>
                        </svg>
                        更改存储位置
                      </button>
                      <button class="settings-btn danger" on:click={resetDatabasePath}>
                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                          <path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8"></path>
                          <path d="M21 3v5h-5"></path>
                          <path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16"></path>
                          <path d="M3 21v-5h5"></path>
                        </svg>
                        重置为默认
                      </button>
                    </div>

                    <div class="database-info">
                      <div class="info-item">
                        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                          <circle cx="12" cy="12" r="10"></circle>
                          <line x1="12" y1="16" x2="12" y2="12"></line>
                          <circle cx="12" cy="8" r="1" fill="currentColor"></circle>
                        </svg>
                        <span>更改数据库位置后需要重启应用才能生效</span>
                      </div>
                      <div class="info-item">
                        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                          <circle cx="12" cy="12" r="10"></circle>
                          <line x1="12" y1="16" x2="12" y2="12"></line>
                          <circle cx="12" cy="8" r="1" fill="currentColor"></circle>
                        </svg>
                        <span>建议选择有足够存储空间的位置</span>
                      </div>
                    </div>
                  </div>
                </div>

                <!-- 邮件预览动画设置 -->
                <div class="setting-item">
                  <div class="setting-header">
                    <h4>邮件预览动画</h4>
                  </div>
                  <p class="setting-description">选择邮件详情窗口的出现动画效果</p>

                  <div class="animation-style-section">
                    <div class="style-options">
                      <label class="style-option" class:active={emailPreviewAnimation === 'slideUp'}>
                        <input type="radio" bind:group={emailPreviewAnimation} value="slideUp" />
                        <div class="option-content">
                          <div class="option-preview animation-preview">
                            <div class="preview-container">
                              <div class="preview-email-card">
                                <div class="preview-card-content">
                                  <div class="preview-subject">邮件主题</div>
                                  <div class="preview-sender">发件人</div>
                                </div>
                              </div>
                              <div class="preview-detail slideup-animation">
                                <div class="preview-detail-header">邮件详情</div>
                                <div class="preview-detail-content">邮件内容预览...</div>
                              </div>
                            </div>
                          </div>
                          <div class="option-info">
                            <div class="option-title">从下往上</div>
                            <div class="option-desc">详情窗口从底部滑入</div>
                          </div>
                        </div>
                      </label>

                      <label class="style-option" class:active={emailPreviewAnimation === 'slideRight'}>
                        <input type="radio" bind:group={emailPreviewAnimation} value="slideRight" />
                        <div class="option-content">
                          <div class="option-preview animation-preview">
                            <div class="preview-container">
                              <div class="preview-email-card">
                                <div class="preview-card-content">
                                  <div class="preview-subject">邮件主题</div>
                                  <div class="preview-sender">发件人</div>
                                </div>
                              </div>
                              <div class="preview-detail slideright-animation">
                                <div class="preview-detail-header">邮件详情</div>
                                <div class="preview-detail-content">邮件内容预览...</div>
                              </div>
                            </div>
                          </div>
                          <div class="option-info">
                            <div class="option-title">从左往右</div>
                            <div class="option-desc">详情窗口从左侧滑入</div>
                          </div>
                        </div>
                      </label>

                      <label class="style-option" class:active={emailPreviewAnimation === 'slideLeft'}>
                        <input type="radio" bind:group={emailPreviewAnimation} value="slideLeft" />
                        <div class="option-content">
                          <div class="option-preview animation-preview">
                            <div class="preview-container">
                              <div class="preview-email-card">
                                <div class="preview-card-content">
                                  <div class="preview-subject">邮件主题</div>
                                  <div class="preview-sender">发件人</div>
                                </div>
                              </div>
                              <div class="preview-detail slideleft-animation">
                                <div class="preview-detail-header">邮件详情</div>
                                <div class="preview-detail-content">邮件内容预览...</div>
                              </div>
                            </div>
                          </div>
                          <div class="option-info">
                            <div class="option-title">从右往左</div>
                            <div class="option-desc">详情窗口从右侧滑入</div>
                          </div>
                        </div>
                      </label>
                    </div>

                    <div class="animation-info">
                      <div class="info-item">
                        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                          <circle cx="12" cy="12" r="10"></circle>
                          <line x1="12" y1="16" x2="12" y2="12"></line>
                          <circle cx="12" cy="8" r="1" fill="currentColor"></circle>
                        </svg>
                        <span>更改动画效果会立即生效</span>
                      </div>
                    </div>
                  </div>
                </div>
              </div>
            {/if}
          </div>
        </div>
      </div>
    {:else if viewMode === 'list'}
      <!-- 邮件列表头部 -->
      <div class="email-list-header">
        <div class="header-top">
          <div class="header-info">
            {#if $currentAccount}
              <div class="account-title-container">
                <span class="account-selection-tag">
                  <span class="tag-text">{getProviderDisplayName(getEmailProvider($currentAccount.email))} - {$currentAccount.email}</span>
                </span>
                <div class="email-stats inline-stats">
                  <span class="total-count">共 {$messages.length} 封邮件</span>
                  <span class="unread-count-text">{$messages.filter(m => !m.is_read).length} 封未读</span>
                </div>
                <!-- 文件夹工具栏展开/收起按钮 -->
                <button
                  class="folder-toggle-btn"
                  on:click={() => isFolderToolbarExpanded = !isFolderToolbarExpanded}
                >
                  <div class="folder-icon">
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <path d="M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z"></path>
                      <polyline points="22,6 12,13 2,6"></polyline>
                    </svg>
                  </div>
                  <span class="folder-toggle-text">{isFolderToolbarExpanded ? '收起' : '文件夹'}</span>
                  <div class="toggle-arrow" class:rotated={isFolderToolbarExpanded}>
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                      <polyline points="6,9 12,15 18,9"></polyline>
                    </svg>
                  </div>
                </button>
              </div>
            {:else}
              <div class="account-title-container">
                <span class="account-selection-tag"><span class="tag-text">请选择邮箱账户</span></span>
                <div class="email-stats inline-stats">
                  <span class="total-count">暂无邮件</span>
                </div>
                <!-- 文件夹工具栏展开/收起按钮 -->
                <button
                  class="folder-toggle-btn disabled"
                  disabled
                >
                  <div class="folder-icon">
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <path d="M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z"></path>
                      <polyline points="22,6 12,13 2,6"></polyline>
                    </svg>
                  </div>
                  <span class="folder-toggle-text">文件夹</span>
                  <div class="toggle-arrow">
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                      <polyline points="6,9 12,15 18,9"></polyline>
                    </svg>
                  </div>
                </button>
              </div>
            {/if}
          </div>
          {#if $currentAccount}
            <div class="header-actions">
              <button class="compose-btn" on:click={openComposeDialog}>
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"></path>
                  <path d="m18.5 2.5 3 3L12 15l-4 1 1-4 9.5-9.5z"></path>
                </svg>
                写邮件
              </button>
              <button class="header-btn" title="全部标记为已读" aria-label="全部标记为已读" on:click={markAllAsRead}>
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <polyline points="20,6 9,17 4,12"></polyline>
                </svg>
              </button>
              <button class="header-btn" title="刷新邮件" aria-label="刷新邮件" on:click={refreshMessages} disabled={isLoadingMessages}>
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class:spinning={isLoadingMessages}>
                  <path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8"></path>
                  <path d="M21 3v5h-5"></path>
                  <path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16"></path>
                  <path d="M3 21v-5h5"></path>
                </svg>
              </button>
            </div>
          {/if}
        </div>


      </div>

      <!-- 卡片布局 -->
      <div class="email-cards-container">
        {#if showClearCacheProgress}
          <div class="clear-cache-progress">
            <div class="progress-header">
              <div class="progress-icon">🗑️</div>
              <div class="progress-title">清空缓存中...</div>
            </div>
            <div class="progress-bar-container">
              <div class="progress-bar">
                <div class="progress-fill" style="width: {clearCacheProgress}%"></div>
              </div>
              <div class="cache-progress-percentage">{clearCacheProgress}%</div>
            </div>
            <div class="progress-step">{clearCacheStep}</div>
          </div>
        {:else if isLoadingMessages}
          <div class="loading-container-wrapper">
            <LoadingProgress currentStep={loadingStep} totalSteps={totalLoadingSteps} isVisible={isLoadingMessages} />
          </div>
        {:else if $messages.length === 0}
          {#if !$currentAccount}
            <div class="simple-welcome">
              <div class="welcome-icon">
                <img src="/src/assets/huanying.png" alt="欢迎" class="welcome-image" />
              </div>

              <p class="welcome-subtitle">快来创建您的邮箱吧！</p>

              <div class="welcome-features">
                <div class="feature-item">
                  <img src="/src/assets/zhichiduozho.png" alt="支持多种邮箱服务商" class="feature-icon-img" />
                  <span>支持多种邮箱服务商</span>
                </div>
                <div class="feature-item">
                  <img src="/src/assets/shishitonbu.png" alt="实时邮件同步" class="feature-icon-img" />
                  <span>实时邮件同步</span>
                </div>
                <div class="feature-item">
                  <img src="/src/assets/anquan.png" alt="安全可靠的邮件管理" class="feature-icon-img" />
                  <span>安全可靠的邮件管理</span>
                </div>
                <div class="feature-item">
                  <img src="/src/assets/zhinengfenlei.png" alt="智能邮件分类" class="feature-icon-img" />
                  <span>智能邮件分类</span>
                </div>
              </div>

              <p class="welcome-hint">点击左侧的邮箱提供商图标开始添加您的第一个邮箱账户</p>
            </div>
          {:else}
            <div class="empty-message">
              <div class="empty-icon">📭</div>
              <p>暂无邮件</p>
            </div>
          {/if}
        {:else}
          {#each $messages as email (email.id)}
            <div
              class="email-card {email.is_read ? 'read' : 'unread'} {email.is_deleted ? 'deleted' : ''}"
              on:click={() => selectEmail(email)}
              on:keydown={(e) => e.key === 'Enter' && selectEmail(email)}
              role="button"
              tabindex="0"
            >
              <div class="card-header">
                <div class="sender-section">
                  <!-- 尝试加载在线头像 -->
                  {#await getSenderAvatarUrl(getCorrectSender(email))}
                    <div class="sender-avatar" style="background-color: #6c7ae0">
                      {getSenderDisplayName(getCorrectSender(email)).charAt(0).toUpperCase()}
                    </div>
                  {:then avatarUrl}
                    {#if avatarUrl}
                      <div class="sender-avatar" style="background-color: transparent">
                        <img
                          src={avatarUrl}
                          alt={getSenderDisplayName(getCorrectSender(email))}
                          class="avatar-image"
                          on:error={() => {
                            // 头像加载失败，缓存已在工具函数中处理
                          }}
                        />
                      </div>
                    {:else}
                      <div class="sender-avatar" style="background-color: #6c7ae0">
                        {getSenderDisplayName(getCorrectSender(email)).charAt(0).toUpperCase()}
                      </div>
                    {/if}
                  {:catch}
                    <div class="sender-avatar" style="background-color: #6c7ae0">
                      {getSenderDisplayName(getCorrectSender(email)).charAt(0).toUpperCase()}
                    </div>
                  {/await}
                  <div class="sender-info">
                    <div class="sender-name">{getSenderDisplayName(getCorrectSender(email))}</div>
                    <div class="sender-email">{getSenderEmail(getCorrectSender(email))}</div>
                  </div>
                </div>
                <div class="card-meta">
                  <div class="send-time">{formatEmailTime(email.received_at)}</div>
                  {#if email.is_starred}
                    <div class="star-indicator">⭐</div>
                  {/if}
                  {#if !email.is_read}
                    <div class="unread-indicator"></div>
                  {/if}
                </div>
              </div>

              <div class="card-content">
                <div class="subject-line">
                  <span class="content-label">主题</span>
                  <h3 class="email-subject">{email.subject || '(无主题)'}</h3>
                </div>
                <div class="content-line">
                  <span class="content-label">内容</span>
                  <div class="email-content-wrapper">
                    <p class="email-preview">{getEmailPreviewWithoutQuote(email)}</p>
                  </div>
                </div>
              </div>

              <div class="card-footer">
                <div class="card-tags">
                  {#await getEmailTags(email)}
                    <span class="folder-tag loading">分析中...</span>
                  {:then tags}
                    {#each tags as tag}
                      <span class="smart-tag {tag.tag_type.toLowerCase()}" style="background-color: {tag.color}; color: white;">
                        {tag.text}
                      </span>
                    {/each}
                  {:catch}
                    <span class="folder-tag error">INBOX</span>
                  {/await}
                  {#if email.is_read}
                    <span class="status-tag read">已读</span>
                  {:else}
                    <span class="status-tag unread">未读</span>
                  {/if}
                </div>
                <div class="card-actions">
                  <!-- 在已删除文件夹中显示还原按钮 -->
                  {#if currentFolder.toLowerCase().includes('trash') || currentFolder.toLowerCase().includes('deleted') || currentFolder.toLowerCase().includes('垃圾')}
                    <button class="card-action-btn restore-btn" title="还原" aria-label="还原邮件" on:click={(e) => restoreEmail(e, email)}>
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                        <path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8"></path>
                        <path d="M21 3v5h-5"></path>
                        <path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16"></path>
                        <path d="M3 21v-5h5"></path>
                      </svg>
                    </button>
                  {:else}
                    <!-- 在其他文件夹中显示收藏和回复按钮 -->
                    <button
                      class="card-action-btn star-btn {email.is_starred ? 'starred' : ''}"
                      title="{email.is_starred ? '取消收藏' : '添加收藏'}"
                      aria-label="{email.is_starred ? '取消收藏邮件' : '收藏邮件'}"
                      on:click={(e) => toggleEmailStar(e, email)}
                    >
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="{email.is_starred ? 'currentColor' : 'none'}" stroke="currentColor" stroke-width="2">
                        <polygon points="12,2 15.09,8.26 22,9.27 17,14.14 18.18,21.02 12,17.77 5.82,21.02 7,14.14 2,9.27 8.91,8.26"></polygon>
                      </svg>
                    </button>
                    <button class="card-action-btn card-reply-btn" title="回复" aria-label="回复邮件" on:click={(e) => { e.stopPropagation(); openReplyDialog(email); }}>
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                        <path d="M3 7l5 5-5 5"></path>
                        <path d="M8 6h13a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H8"></path>
                      </svg>
                    </button>
                  {/if}

                  <!-- 删除按钮在所有文件夹中都显示 -->
                  <button class="card-action-btn delete-btn" title="删除" aria-label="删除邮件" on:click={(e) => showDeleteConfirmDialog(e, email)}>
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <path d="M3 6h18"></path>
                      <path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"></path>
                      <path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"></path>
                    </svg>
                  </button>
                </div>
              </div>
            </div>
          {/each}
        {/if}
      </div>
    {:else if viewMode === 'timeline'}
      <!-- 时间线视图 -->
      <div class="timeline-container">
        {#if $currentAccount && $messages.length > 0}
          <div class="timeline-line"></div>
        {/if}
        {#if showClearCacheProgress}
          <div class="clear-cache-progress">
            <div class="progress-header">
              <div class="progress-icon">🗑️</div>
              <div class="progress-title">清空缓存中...</div>
            </div>
            <div class="progress-bar-container">
              <div class="progress-bar">
                <div class="progress-fill" style="width: {clearCacheProgress}%"></div>
              </div>
              <div class="cache-progress-percentage">{clearCacheProgress}%</div>
            </div>
            <div class="progress-step">{clearCacheStep}</div>
          </div>
        {:else if isLoadingMessages}
          <div class="loading-container-wrapper">
            <LoadingProgress currentStep={loadingStep} totalSteps={totalLoadingSteps} isVisible={isLoadingMessages} />
          </div>
        {:else if $messages.length === 0}
          {#if !$currentAccount}
            <div class="simple-welcome">
              <div class="welcome-icon">
                <img src="/src/assets/huanying.png" alt="欢迎" class="welcome-image" />
              </div>

              <p class="welcome-subtitle">快来创建您的邮箱吧！</p>

              <div class="welcome-features">
                <div class="feature-item">
                  <img src="/src/assets/zhichiduozho.png" alt="支持多种邮箱服务商" class="feature-icon-img" />
                  <span>支持多种邮箱服务商</span>
                </div>
                <div class="feature-item">
                  <img src="/src/assets/shishitonbu.png" alt="实时邮件同步" class="feature-icon-img" />
                  <span>实时邮件同步</span>
                </div>
                <div class="feature-item">
                  <img src="/src/assets/anquan.png" alt="安全可靠的邮件管理" class="feature-icon-img" />
                  <span>安全可靠的邮件管理</span>
                </div>
                <div class="feature-item">
                  <img src="/src/assets/zhinengfenlei.png" alt="智能邮件分类" class="feature-icon-img" />
                  <span>智能邮件分类</span>
                </div>
              </div>

              <p class="welcome-hint">点击左侧的邮箱提供商图标开始添加您的第一个邮箱账户</p>
            </div>
          {:else}
            <div class="empty-message">
              <div class="empty-icon">📭</div>
              <p>暂无邮件</p>
            </div>
          {/if}
        {:else}
          {#each $messages as email, index (email.id)}
            <div class="timeline-item {index % 2 === 0 ? 'left' : 'right'} {email.is_deleted ? 'deleted' : ''}">
              {#if email.is_deleted}
                <div class="timeline-marker" style="background-color: #adb5bd">
                  {getSenderDisplayName(email.sender).charAt(0).toUpperCase()}
                </div>
              {:else}
                {#await getSenderAvatarUrl(email.sender)}
                  <div class="timeline-marker" style="background-color: #6c7ae0">
                    {getSenderDisplayName(email.sender).charAt(0).toUpperCase()}
                  </div>
                {:then avatarUrl}
                  {#if avatarUrl}
                    <div class="timeline-marker" style="background-color: transparent">
                      <img
                        src={avatarUrl}
                        alt={getSenderDisplayName(email.sender)}
                        class="timeline-avatar-image"
                      />
                    </div>
                  {:else}
                    <div class="timeline-marker" style="background-color: #6c7ae0">
                      {getSenderDisplayName(email.sender).charAt(0).toUpperCase()}
                    </div>
                  {/if}
                {:catch}
                  <div class="timeline-marker" style="background-color: #6c7ae0">
                    {getSenderDisplayName(email.sender).charAt(0).toUpperCase()}
                  </div>
                {/await}
              {/if}
              <div class="timeline-content">
                <div class="timeline-time">{formatEmailTime(email.received_at)}</div>
                <h4>{email.subject || '(无主题)'} {email.is_deleted ? '🗑️' : ''}</h4>
                <p class="timeline-from">来自: {getSenderDisplayName(email.sender)}</p>
                <p class="timeline-preview">{getEmailPreviewWithoutQuote(email)}</p>
              </div>
            </div>
          {/each}
        {/if}
      </div>
    {:else if viewMode === 'chat'}
      <!-- 聊天气泡视图 -->
      <div class="chat-container">
        {#if showClearCacheProgress}
          <div class="clear-cache-progress">
            <div class="progress-header">
              <div class="progress-icon">🗑️</div>
              <div class="progress-title">清空缓存中...</div>
            </div>
            <div class="progress-bar-container">
              <div class="progress-bar">
                <div class="progress-fill" style="width: {clearCacheProgress}%"></div>
              </div>
              <div class="cache-progress-percentage">{clearCacheProgress}%</div>
            </div>
            <div class="progress-step">{clearCacheStep}</div>
          </div>
        {:else if isLoadingMessages}
          <div class="loading-container-wrapper">
            <LoadingProgress currentStep={loadingStep} totalSteps={totalLoadingSteps} isVisible={isLoadingMessages} />
          </div>
        {:else if $messages.length === 0}
          {#if !$currentAccount}
            <div class="simple-welcome">
              <div class="welcome-icon">
                <img src="/src/assets/huanying.png" alt="欢迎" class="welcome-image" />
              </div>

              <p class="welcome-subtitle">快来创建您的邮箱吧！</p>

              <div class="welcome-features">
                <div class="feature-item">
                  <img src="/src/assets/zhichiduozho.png" alt="支持多种邮箱服务商" class="feature-icon-img" />
                  <span>支持多种邮箱服务商</span>
                </div>
                <div class="feature-item">
                  <img src="/src/assets/shishitonbu.png" alt="实时邮件同步" class="feature-icon-img" />
                  <span>实时邮件同步</span>
                </div>
                <div class="feature-item">
                  <img src="/src/assets/anquan.png" alt="安全可靠的邮件管理" class="feature-icon-img" />
                  <span>安全可靠的邮件管理</span>
                </div>
                <div class="feature-item">
                  <img src="/src/assets/zhinengfenlei.png" alt="智能邮件分类" class="feature-icon-img" />
                  <span>智能邮件分类</span>
                </div>
              </div>

              <p class="welcome-hint">点击左侧的邮箱提供商图标开始添加您的第一个邮箱账户</p>
            </div>
          {:else}
            <div class="empty-message">
              <div class="empty-icon">📭</div>
              <p>暂无邮件</p>
            </div>
          {/if}
        {:else}
          {#each $messages as email (email.id)}
            <div class="chat-message {email.is_deleted ? 'deleted' : ''}">
              {#if email.is_deleted}
                <div class="message-avatar" style="background-color: #adb5bd">
                  {getSenderDisplayName(email.sender).charAt(0).toUpperCase()}
                </div>
              {:else}
                {#await getSenderAvatarUrl(email.sender)}
                  <div class="message-avatar" style="background-color: #6c7ae0">
                    {getSenderDisplayName(email.sender).charAt(0).toUpperCase()}
                  </div>
                {:then avatarUrl}
                  {#if avatarUrl}
                    <div class="message-avatar" style="background-color: transparent">
                      <img
                        src={avatarUrl}
                        alt={getSenderDisplayName(email.sender)}
                        class="chat-avatar-image"
                      />
                    </div>
                  {:else}
                    <div class="message-avatar" style="background-color: #6c7ae0">
                      {getSenderDisplayName(email.sender).charAt(0).toUpperCase()}
                    </div>
                  {/if}
                {:catch}
                  <div class="message-avatar" style="background-color: #6c7ae0">
                    {getSenderDisplayName(email.sender).charAt(0).toUpperCase()}
                  </div>
                {/await}
              {/if}
              <div class="message-bubble">
                <div class="message-header">
                  <span class="message-sender">{getSenderDisplayName(email.sender)}</span>
                  <span class="message-time">{formatEmailTime(email.received_at)}</span>
                  {#if email.is_deleted}<span class="deleted-indicator">🗑️ 已删除</span>{/if}
                </div>
                <div class="message-subject">{email.subject || '(无主题)'}</div>
                <div class="message-content">{getEmailPreviewWithoutQuote(email)}</div>
              </div>
            </div>
          {/each}
        {/if}
      </div>
    {/if}
      </div>

    <!-- 悬浮的邮件文件夹导航 -->
    {#if $currentAccount && isFolderToolbarExpanded}
      <div class="floating-folder-toolbar">
        <div class="email-folders">
          <button class="folder-btn {currentFolder === 'inbox' ? 'active' : ''}" on:click={() => setCurrentFolder('inbox')}>
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z"></path>
              <polyline points="22,6 12,13 2,6"></polyline>
            </svg>
            收件箱
          </button>
          <button class="folder-btn {currentFolder === 'sent' ? 'active' : ''}" on:click={() => setCurrentFolder('sent')}>
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <line x1="22" y1="2" x2="11" y2="13"></line>
              <polygon points="22,2 15,22 11,13 2,9"></polygon>
            </svg>
            已发送
          </button>
          <button class="folder-btn {currentFolder === 'drafts' ? 'active' : ''}" on:click={() => setCurrentFolder('drafts')}>
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"></path>
              <path d="m18.5 2.5 3 3L12 15l-4 1 1-4 9.5-9.5z"></path>
            </svg>
            草稿
          </button>
          <button class="folder-btn {currentFolder === 'spam' ? 'active' : ''}" on:click={() => setCurrentFolder('spam')}>
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <circle cx="12" cy="12" r="10"></circle>
              <line x1="4.93" y1="4.93" x2="19.07" y2="19.07"></line>
            </svg>
            垃圾邮件
          </button>
          <button class="folder-btn {currentFolder === 'trash' ? 'active' : ''}" on:click={() => setCurrentFolder('trash')}>
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="3,6 5,6 21,6"></polyline>
              <path d="m19,6v14a2,2 0 0,1-2,2H7a2,2 0 0,1-2-2V6m3,0V4a2,2 0 0,1,2-2h4a2,2 0 0,1,2,2v2"></path>
            </svg>
            已删除
          </button>
          <button class="folder-btn {currentFolder === 'starred' ? 'active' : ''}" on:click={() => setCurrentFolder('starred')}>
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polygon points="12,2 15.09,8.26 22,9.27 17,14.14 18.18,21.02 12,17.77 5.82,21.02 7,14.14 2,9.27 8.91,8.26"></polygon>
            </svg>
            收藏
          </button>
        </div>
      </div>
    {/if}
    </main>

    <!-- 邮件详情侧边栏 -->
    {#if selectedEmail}
      <aside class="email-detail-sidebar {emailPreviewAnimation === 'slideRight' ? 'slide-right' : emailPreviewAnimation === 'slideLeft' ? 'slide-left' : 'slide-up'}">
        <!-- 邮件头部信息 -->
        <div class="email-header">
          <div class="email-meta">
            <div class="sender-info">
              {#await getSenderAvatarUrl(getCorrectSender(selectedEmail))}
                <div class="sender-avatar" style="background: linear-gradient(135deg, var(--theme-primary), var(--theme-secondary))">
                  {getSenderDisplayName(getCorrectSender(selectedEmail)).charAt(0).toUpperCase()}
                </div>
              {:then avatarUrl}
                {#if avatarUrl}
                  <div class="sender-avatar">
                    <img
                      src={avatarUrl}
                      alt={getSenderDisplayName(getCorrectSender(selectedEmail))}
                      loading="lazy"
                      on:error={(e) => {
                        const target = e.target as HTMLImageElement;
                        const parent = target?.parentElement as HTMLElement;
                        if (target && parent && selectedEmail) {
                          target.style.display = 'none';
                          parent.innerHTML = getSenderDisplayName(getCorrectSender(selectedEmail)).charAt(0).toUpperCase();
                          parent.style.background = 'linear-gradient(135deg, var(--theme-primary), var(--theme-secondary))';
                          parent.style.color = 'white';
                          parent.style.display = 'flex';
                          parent.style.alignItems = 'center';
                          parent.style.justifyContent = 'center';
                          parent.style.fontSize = '18px';
                          parent.style.fontWeight = '600';
                        }
                      }}
                    />
                  </div>
                {:else}
                  <div class="sender-avatar" style="background: linear-gradient(135deg, var(--theme-primary), var(--theme-secondary))">
                    {getSenderDisplayName(getCorrectSender(selectedEmail)).charAt(0).toUpperCase()}
                  </div>
                {/if}
              {/await}

              <div class="sender-details">
                <h3 class="sender-name">{getSenderDisplayName(getCorrectSender(selectedEmail))}</h3>
                <p class="sender-email">{getCorrectSender(selectedEmail)}</p>
                <p class="email-time">{formatEmailTime(selectedEmail.received_at)}</p>
              </div>
            </div>

            <div class="email-actions">
              <button class="email-action-btn action-reply-btn" on:click={() => openReplyDialog(selectedEmail!)} title="回复" aria-label="回复邮件">
                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M3 7l5 5-5 5"></path>
                  <path d="M8 6h13a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H8"></path>
                </svg>
              </button>
              <button class="email-action-btn forward-btn" on:click={() => openForwardDialog(selectedEmail!)} title="转发" aria-label="转发邮件">
                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="m15 17 5-5-5-5"></path>
                  <path d="M4 18v-2a4 4 0 0 1 4-4h12"></path>
                </svg>
              </button>
              <button class="email-action-btn delete-btn" on:click={(e) => showDeleteConfirmDialog(e, selectedEmail!)} title="删除" aria-label="删除邮件">
                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <polyline points="3,6 5,6 21,6"></polyline>
                  <path d="m19,6v14a2,2 0 0,1-2,2H7a2,2 0 0,1-2-2V6m3,0V4a2,2 0 0,1,2-2h4a2,2 0 0,1,2,2v2"></path>
                </svg>
              </button>
              <button class="email-action-btn translate-btn" on:click={() => translateEmail(selectedEmail!)} title="翻译" aria-label="翻译邮件" disabled={isTranslating}>
                {#if isTranslating}
                  <!-- 翻译中的加载动画 -->
                  <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="animate-spin">
                    <path d="M21 12a9 9 0 11-6.219-8.56"></path>
                  </svg>
                {:else}
                  <!-- 翻译图标 -->
                  <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M5 8l6 6"></path>
                    <path d="M4 14l6-6 2-3"></path>
                    <path d="M2 5h12"></path>
                    <path d="M7 2h1"></path>
                    <path d="M22 22l-5-10-5 10"></path>
                    <path d="M14 18h6"></path>
                  </svg>
                {/if}
              </button>

              <!-- 翻译进度提示 -->
              {#if selectedEmail && translatedEmails[selectedEmail.id]?.isTranslating}
                <div class="translation-progress">
                  <div class="progress-spinner"></div>
                  <span class="translation-progress-text">正在翻译中...</span>
                </div>
              {/if}
              <button class="email-action-btn" on:click={() => selectedEmail = null} title="关闭" aria-label="关闭邮件详情">
                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <line x1="18" y1="6" x2="6" y2="18"></line>
                  <line x1="6" y1="6" x2="18" y2="18"></line>
                </svg>
              </button>
            </div>
          </div>

          <div class="email-subject">
            <h2>{selectedEmail.subject || '(无主题)'}</h2>
          </div>
        </div>

        <!-- 次要操作按钮 -->
        <div class="secondary-actions">{#if selectedEmail && !selectedEmail.is_read}
              <button class="secondary-btn read-btn" on:click={() => markEmailAsRead(selectedEmail!)} title="标记为已读">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <polyline points="20,6 9,17 4,12"></polyline>
                </svg>
                标记为已读
              </button>
            {/if}

            {#if selectedEmail && selectedEmail.is_read}
              <button class="secondary-btn unread-btn" on:click={() => markEmailAsUnread(selectedEmail!)} title="标记为未读">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <circle cx="12" cy="12" r="10"></circle>
                  <path d="M8 12h8"></path>
                </svg>
                标记为未读
              </button>
            {/if}



            <!-- 垃圾邮件文件夹中显示"这不是垃圾邮件"按钮 -->
            {#if currentFolder === 'spam' && selectedEmail}
              <button class="secondary-btn not-spam-btn" on:click={() => moveOutOfSpam(selectedEmail!)} title="这不是垃圾邮件">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M9 12l2 2 4-4"></path>
                  <circle cx="12" cy="12" r="10"></circle>
                </svg>
                不是垃圾邮件
              </button>
            {/if}

            <!-- 非垃圾邮件文件夹中显示"标记为垃圾邮件"按钮 -->
            {#if currentFolder !== 'spam' && selectedEmail}
              <button class="secondary-btn spam-btn" on:click={() => markAsSpam(selectedEmail!)} title="标记为垃圾邮件">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M3 6h18l-2 13H5L3 6z"></path>
                  <path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path>
                  <line x1="10" y1="11" x2="10" y2="17"></line>
                  <line x1="14" y1="11" x2="14" y2="17"></line>
                </svg>
                标记为垃圾邮件
              </button>
            {/if}
        </div>

        <!-- 邮件内容 -->
        <div class="email-content">
          <!-- 翻译状态指示器 -->
          {#if selectedEmail && translatedEmails[selectedEmail.id]}
            <div class="translation-indicator">
              <div class="translation-status">
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M5 8l6 6"></path>
                  <path d="M4 14l6-6 2-3"></path>
                  <path d="M2 5h12"></path>
                  <path d="M7 2h1"></path>
                  <path d="M22 22l-5-10-5 10"></path>
                  <path d="M14 18h6"></path>
                </svg>
                <span class="translation-text">
                  翻译为 {translatedEmails[selectedEmail.id]?.sourceLanguage === 'zh' ? '英文' : '中文'}
                </span>
              </div>
              <button
                class="show-original-btn"
                on:click={() => selectedEmail && toggleTranslation(selectedEmail.id)}
                title="{showOriginal[selectedEmail.id] ? '显示翻译' : '显示原始邮件'}"
                aria-label="{showOriginal[selectedEmail.id] ? '显示翻译' : '显示原始邮件'}"
              >
                {showOriginal[selectedEmail.id] ? '显示翻译' : '显示原始邮件'}
              </button>
            </div>
          {/if}

          <!-- 邮件正文 -->
          <div class="email-body">
            {#if selectedEmail}
              {#if translatedEmails[selectedEmail.id] && !showOriginal[selectedEmail.id]}
                <!-- 显示翻译内容 -->
                {#if translatedEmails[selectedEmail.id].body_html}
                  {@html translatedEmails[selectedEmail.id].body_html}
                {:else if translatedEmails[selectedEmail.id].body_text}
                  <pre class="text-content">{translatedEmails[selectedEmail.id].body_text}</pre>
                {:else}
                  <p class="no-content">此邮件没有内容</p>
                {/if}
              {:else}
                <!-- 显示原始内容 -->
                {#if selectedEmail.body_html}
                  {@html selectedEmail.body_html}
                {:else if selectedEmail.body_text}
                  <pre class="text-content">{selectedEmail.body_text}</pre>
                {:else}
                  <p class="no-content">此邮件没有内容</p>
                {/if}
              {/if}
            {/if}
          </div>
        </div>
      </aside>
    {/if}
  </div>
</div>

<!-- 自定义主题对话框 -->
{#if showCustomThemeDialog}
  <div
    class="dialog-overlay"
    role="dialog"
    aria-modal="true"
    tabindex="-1"
    on:click={closeCustomThemeDialog}
    on:keydown={(e) => e.key === 'Escape' && closeCustomThemeDialog()}
  >
    <!-- svelte-ignore a11y-no-noninteractive-element-interactions -->
    <!-- svelte-ignore a11y-click-events-have-key-events -->
    <div
      class="custom-theme-dialog show"
      role="document"
      on:click|stopPropagation
      on:keydown|stopPropagation
    >
      <div class="dialog-header">
        <h3>自定义主题</h3>
      </div>

      <div class="dialog-content">
        <div class="theme-preview-section">
          <h4>预览效果</h4>
          <div class="theme-preview-large">
            <div class="preview-card" style="background: linear-gradient(135deg, {customTheme.primary}, {customTheme.secondary})">
              <div class="preview-content">
                <div class="preview-title">主题预览</div>
                <div class="preview-button" style="background: {customTheme.primary}; border-color: {customTheme.hover}">
                  主要按钮
                </div>
                <div class="preview-button secondary" style="background: {customTheme.secondary}; border-color: {customTheme.hoverSecondary}">
                  次要按钮
                </div>
              </div>
            </div>
          </div>
        </div>

        <div class="color-settings">
          <div class="color-warning">
            <div class="warning-icon">⚠️</div>
            <div class="warning-text">
              <strong>注意：</strong>请避免使用过浅的颜色（如白色、浅灰色等），这会导致按钮与背景难以区分。系统会自动调整过浅的颜色。
            </div>
          </div>

          <div class="color-group">
            <label for="primary-color">主要颜色</label>
            <div class="color-input-group">
              <input
                id="primary-color"
                type="color"
                bind:value={customTheme.primary}
                on:input={onPrimaryColorChange}
                class="color-picker"
              />
              <input
                type="text"
                bind:value={customTheme.primary}
                on:input={onPrimaryColorChange}
                class="color-text-input"
                placeholder="#6c7ae0"
              />
            </div>
          </div>

          <div class="color-group">
            <label for="secondary-color">次要颜色</label>
            <div class="color-input-group">
              <input
                id="secondary-color"
                type="color"
                bind:value={customTheme.secondary}
                on:input={onSecondaryColorChange}
                class="color-picker"
              />
              <input
                type="text"
                bind:value={customTheme.secondary}
                on:input={onSecondaryColorChange}
                class="color-text-input"
                placeholder="#7b68ee"
              />
            </div>
          </div>

          <div class="color-group">
            <label for="hover-color">主要悬停色</label>
            <div class="color-input-group">
              <input
                id="hover-color"
                type="color"
                bind:value={customTheme.hover}
                class="color-picker"
              />
              <input
                type="text"
                bind:value={customTheme.hover}
                class="color-text-input"
                placeholder="#5a68d4"
              />
            </div>
          </div>

          <div class="color-group">
            <label for="hover-secondary-color">次要悬停色</label>
            <div class="color-input-group">
              <input
                id="hover-secondary-color"
                type="color"
                bind:value={customTheme.hoverSecondary}
                class="color-picker"
              />
              <input
                type="text"
                bind:value={customTheme.hoverSecondary}
                class="color-text-input"
                placeholder="#6c5ce7"
              />
            </div>
          </div>

          <div class="color-group">
            <label for="theme-name">主题名称</label>
            <input
              id="theme-name"
              type="text"
              bind:value={customTheme.name}
              class="theme-name-input"
              placeholder="自定义主题"
            />
          </div>
        </div>
      </div>

      <div class="dialog-footer">
        <button class="reset-btn" on:click={resetCustomTheme}>
          重置为默认
        </button>
        <div class="action-buttons">
          <button class="cancel-btn" on:click={closeCustomThemeDialog}>
            取消
          </button>
          <button class="apply-btn" on:click={applyCustomTheme}>
            应用主题
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}

<!-- 删除确认对话框 -->
{#if showDeleteConfirm}
  <div
    class="delete-confirm-overlay"
    role="dialog"
    aria-modal="true"
    aria-labelledby="dialog-title"
    on:click={handleOverlayClick}
    on:keydown={handleDialogKeydown}
    tabindex="-1"
    use:focusDialog
  >
    <div
      class="delete-confirm-dialog"
      role="document"
    >
      <div class="dialog-header">
        <div class="dialog-icon">
          <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M3 6h18"></path>
            <path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"></path>
            <path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"></path>
          </svg>
        </div>
        <h3 class="dialog-title" id="dialog-title">
          {isInTrashFolder ? '永久删除邮件' : '删除邮件'}
        </h3>
      </div>

      <div class="dialog-content">
        <p class="dialog-message">{deleteConfirmMessage}</p>
      </div>

      <div class="dialog-actions">
        <button class="dialog-btn cancel-btn" on:click={cancelDelete}>
          取消
        </button>
        <button class="dialog-btn confirm-btn" on:click={confirmDeleteEmail}>
          {isInTrashFolder ? '永久删除' : '删除'}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- 数据库路径选择对话框 -->
{#if showDatabasePathDialog}
  <div
    class="database-path-overlay"
    role="dialog"
    aria-modal="true"
    aria-labelledby="database-path-title"
    on:click={(e) => e.target === e.currentTarget && (showDatabasePathDialog = false)}
    on:keydown={(e) => e.key === 'Escape' && (showDatabasePathDialog = false)}
    tabindex="-1"
  >
    <div class="database-path-dialog">
      <div class="dialog-header">
        <h3 id="database-path-title">
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M3 7v10a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V9a2 2 0 0 0-2-2H5a2 2 0 0 0-2-2z"></path>
            <path d="M8 21v-4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v4"></path>
          </svg>
          更改数据库存储位置
        </h3>
      </div>

      <div class="dialog-content">
        <p class="dialog-description">选择新的SQLite数据库存储位置。更改后需要重启应用才能生效。</p>

        <div class="path-input-section">
          <label for="custom-path">新的存储路径：</label>
          <div class="path-input-group">
            <input
              type="text"
              id="custom-path"
              bind:value={customDatabasePath}
              placeholder="请选择数据库存储路径"
              readonly
            />
            <button class="browse-btn" on:click={selectDatabasePath}>
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M3 7v10a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V9a2 2 0 0 0-2-2H5a2 2 0 0 0-2-2z"></path>
                <path d="M8 21v-4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v4"></path>
              </svg>
              浏览
            </button>
          </div>
        </div>

        <div class="migration-options">
          <div class="option-item">
            <input type="radio" id="migrate-data" name="migration" value="migrate" bind:group={migrationOption} />
            <label for="migrate-data">
              <div class="option-content">
                <div class="option-title">
                  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path>
                    <polyline points="14,2 14,8 20,8"></polyline>
                    <line x1="16" y1="13" x2="8" y2="13"></line>
                    <line x1="16" y1="17" x2="8" y2="17"></line>
                    <polyline points="10,9 9,9 8,9"></polyline>
                  </svg>
                  迁移现有数据（推荐）
                </div>
                <div class="option-description">将现有邮件数据复制到新位置，保留所有邮件和设置</div>
              </div>
            </label>
          </div>

          <div class="option-item">
            <input type="radio" id="fresh-start" name="migration" value="fresh" bind:group={migrationOption} />
            <label for="fresh-start">
              <div class="option-content">
                <div class="option-title">
                  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8"></path>
                    <path d="M21 3v5h-5"></path>
                    <path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16"></path>
                    <path d="M3 21v-5h5"></path>
                  </svg>
                  重新开始
                </div>
                <div class="option-description">在新位置创建空数据库，不保留现有邮件缓存</div>
              </div>
            </label>
          </div>
        </div>

        <div class="info-section">
          <div class="info-item">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <circle cx="12" cy="12" r="10"></circle>
              <line x1="12" y1="16" x2="12" y2="12"></line>
              <circle cx="12" cy="8" r="1" fill="currentColor"></circle>
            </svg>
            <span>更改后需要重启应用才能生效</span>
          </div>
          <div class="info-item">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <circle cx="12" cy="12" r="10"></circle>
              <line x1="12" y1="16" x2="12" y2="12"></line>
              <circle cx="12" cy="8" r="1" fill="currentColor"></circle>
            </svg>
            <span>请确保选择的位置有足够的存储空间</span>
          </div>
        </div>
      </div>

      <div class="dialog-actions">
        <button class="dialog-btn cancel-btn" on:click={() => showDatabasePathDialog = false}>
          取消
        </button>
        <button class="dialog-btn confirm-btn" on:click={applyDatabasePath} disabled={!customDatabasePath.trim()}>
          应用设置
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- 回复邮件对话框 -->
{#if showReplyDialog && replyToEmail}
  <div
    class="reply-dialog-overlay"
    role="dialog"
    aria-modal="true"
    aria-labelledby="reply-dialog-title"
    on:keydown={(e) => e.key === 'Escape' && closeReplyDialog()}
    tabindex="-1"
  >
    <!-- 点击关闭区域 -->
    <button class="overlay-close-btn" on:click={closeReplyDialog} aria-label="关闭对话框"></button>

    <div class="reply-dialog">
      <div class="reply-header">
        <h3 id="reply-dialog-title">回复邮件</h3>
      </div>

      <div class="reply-content">
        <!-- 顶部基本信息 -->
        <div class="reply-header-fields">
          <div class="reply-field">
            <label for="reply-to">收件人:</label>
            <input type="text" id="reply-to" value={getReplyToAddress(replyToEmail)} readonly />
          </div>

          <div class="reply-field">
            <label for="reply-subject">主题:</label>
            <input type="text" id="reply-subject" value={replySubject} readonly />
          </div>
        </div>

        <!-- 左右分栏布局 -->
        <div class="reply-main-content">
          <!-- 左侧：用户回复区 -->
          <div class="reply-compose-area">
            <div class="reply-field">
              <label for="reply-body">您的回复:</label>

              <!-- 回复富文本编辑器工具栏 -->
              <div class="reply-editor-toolbar">
                <!-- 字体格式 -->
                <div class="toolbar-group">
                  <button type="button" class="editor-btn" on:click={() => replyFormatText('bold')} title="粗体">
                    <strong>B</strong>
                  </button>
                  <button type="button" class="editor-btn" on:click={() => replyFormatText('italic')} title="斜体">
                    <em>I</em>
                  </button>
                  <button type="button" class="editor-btn" on:click={() => replyFormatText('underline')} title="下划线">
                    <u>U</u>
                  </button>
                  <button type="button" class="editor-btn" on:click={() => replyFormatText('strikeThrough')} title="删除线">
                    <s>S</s>
                  </button>
                </div>

                <!-- 字体大小 -->
                <div class="toolbar-group">
                  <div class="dropdown">
                    <button type="button" class="editor-btn dropdown-btn" on:click={() => showReplyFontSizePicker = !showReplyFontSizePicker} title="字体大小">
                      字号
                    </button>
                    {#if showReplyFontSizePicker}
                      <div class="dropdown-menu">
                        <button type="button" on:click={() => setReplyFontSize('1')}>小</button>
                        <button type="button" on:click={() => setReplyFontSize('3')}>正常</button>
                        <button type="button" on:click={() => setReplyFontSize('5')}>大</button>
                        <button type="button" on:click={() => setReplyFontSize('7')}>特大</button>
                      </div>
                    {/if}
                  </div>
                </div>

                <!-- 颜色 -->
                <div class="toolbar-group">
                  <div class="dropdown">
                    <button type="button" class="editor-btn dropdown-btn" on:click={() => showReplyColorPicker = !showReplyColorPicker} title="文字颜色">
                      🎨
                    </button>
                    {#if showReplyColorPicker}
                      <div class="dropdown-menu color-picker">
                        <button type="button" class="color-btn" style="background: #000000" on:click={() => setReplyTextColor('#000000')} aria-label="黑色"></button>
                        <button type="button" class="color-btn" style="background: #ff0000" on:click={() => setReplyTextColor('#ff0000')} aria-label="红色"></button>
                        <button type="button" class="color-btn" style="background: #00ff00" on:click={() => setReplyTextColor('#00ff00')} aria-label="绿色"></button>
                        <button type="button" class="color-btn" style="background: #0000ff" on:click={() => setReplyTextColor('#0000ff')} aria-label="蓝色"></button>
                        <button type="button" class="color-btn" style="background: #ffff00" on:click={() => setReplyTextColor('#ffff00')} aria-label="黄色"></button>
                        <button type="button" class="color-btn" style="background: #ff00ff" on:click={() => setReplyTextColor('#ff00ff')} aria-label="紫色"></button>
                        <button type="button" class="color-btn" style="background: #00ffff" on:click={() => setReplyTextColor('#00ffff')} aria-label="青色"></button>
                        <button type="button" class="color-btn" style="background: #808080" on:click={() => setReplyTextColor('#808080')} aria-label="灰色"></button>
                      </div>
                    {/if}
                  </div>
                </div>

                <!-- 对齐 -->
                <div class="toolbar-group">
                  <button type="button" class="editor-btn" on:click={() => replyFormatText('justifyLeft')} title="左对齐">
                    ⬅️
                  </button>
                  <button type="button" class="editor-btn" on:click={() => replyFormatText('justifyCenter')} title="居中">
                    ↔️
                  </button>
                  <button type="button" class="editor-btn" on:click={() => replyFormatText('justifyRight')} title="右对齐">
                    ➡️
                  </button>
                </div>

                <!-- 列表 -->
                <div class="toolbar-group">
                  <button type="button" class="editor-btn" on:click={() => insertReplyList('ul')} title="无序列表">
                    • 列表
                  </button>
                  <button type="button" class="editor-btn" on:click={() => insertReplyList('ol')} title="有序列表">
                    1. 列表
                  </button>
                </div>

                <!-- 链接和图片 -->
                <div class="toolbar-group">
                  <button type="button" class="editor-btn" on:click={insertReplyLink} title="插入链接">
                    🔗
                  </button>
                  <button type="button" class="editor-btn" on:click={insertReplyImage} title="插入图片">
                    🖼️
                  </button>
                </div>
              </div>

              <!-- 回复富文本编辑器 -->
              <div
                bind:this={replyEditorElement}
                class="reply-rich-editor"
                contenteditable="true"
                data-placeholder="请在此输入您的回复内容..."
                on:input={() => replyBody = replyEditorElement?.textContent || ''}
              ></div>
            </div>
          </div>

          <!-- 右侧：原始邮件引用 -->
          <div class="original-message-area">
            <div class="original-message">
              <div class="original-message-header">
                <div class="separator-line"></div>
              </div>
              <div class="original-message-content">
                <div class="original-message-meta">
                  {#if $currentAccount?.email.includes('@gmail.com')}
                    <div><strong>From:</strong> {replyToEmail.sender}</div>
                    <div><strong>Sent:</strong> {new Date(replyToEmail.received_at).toLocaleString('en-US', {
                      weekday: 'long',
                      year: 'numeric',
                      month: 'long',
                      day: 'numeric',
                      hour: 'numeric',
                      minute: '2-digit',
                      hour12: true
                    })}</div>
                    <div><strong>To:</strong> {getOriginalRecipients(replyToEmail)}</div>
                    <div><strong>Subject:</strong> {replyToEmail.subject}</div>
                  {:else}
                    <div><strong>发件人:</strong> {replyToEmail.sender}</div>
                    <div><strong>发送时间:</strong> {new Date(replyToEmail.received_at).toLocaleString('zh-CN')}</div>
                    <div><strong>收件人:</strong> {getOriginalRecipients(replyToEmail)}</div>
                    <div><strong>主题:</strong> {replyToEmail.subject}</div>
                  {/if}
                </div>
                <div class="original-message-body">
                  {#if replyToEmail.body_html && replyToEmail.body_html.trim()}
                    <div class="html-content">
                      {@html sanitizeHtml(replyToEmail.body_html)}
                    </div>
                  {:else if replyToEmail.body_text && replyToEmail.body_text.trim()}
                    <div class="text-content">
                      <pre>{replyToEmail.body_text}</pre>
                    </div>
                  {:else}
                    <div class="no-content-detail">
                      <div class="no-content-icon">📭</div>
                      <h4>此邮件没有文本内容</h4>
                      <p>这封邮件可能只包含附件或其他媒体内容</p>
                    </div>
                  {/if}
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <div class="reply-actions">
        <button class="reply-btn cancel" on:click={closeReplyDialog}>取消</button>
        <button class="reply-btn send" on:click={sendReply}>发送</button>
      </div>
    </div>
  </div>
{/if}

<!-- 转发邮件对话框 -->
{#if showForwardDialog && forwardEmail}
  <ComposeEmailDialog
    bind:show={showForwardDialog}
    on:close={() => {
      closeForwardDialog();
    }}
    initialTo=""
    initialSubject="转发: {forwardEmail.subject}"
    initialBody={cachedForwardContent}
    isForwardMode={true}
    forwardEmail={forwardEmail}
  />
{/if}

<!-- 写邮件对话框 -->
<ComposeEmailDialog bind:show={showComposeDialog} />

<!-- 保存草稿确认对话框 -->
{#if showSaveDraftConfirm}
  <div class="save-draft-overlay" role="dialog" aria-modal="true" tabindex="-1">
    <div class="save-draft-dialog">
      <div class="save-draft-header">
        <h3>保存草稿</h3>
      </div>

      <div class="save-draft-content">
        <p class="save-draft-message">您的回复内容尚未发送，是否保存为草稿？</p>
      </div>

      <div class="save-draft-actions">
        <button class="save-draft-btn discard-btn" on:click={discardReplyDraft}>
          不保存
        </button>
        <button class="save-draft-btn confirm-btn" on:click={saveReplyAsDraft}>
          保存草稿
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  /* CSS变量定义 */
  :global(:root) {
    --theme-primary: #6c7ae0;
    --theme-secondary: #7b68ee;
    --theme-hover: #5a68d4;
    --theme-hover-secondary: #6c5ce7;
    --theme-primary-rgb: 108, 122, 224;
  }

  /* 减少可能导致窗口重绘的动画效果 */
  :global(*) {
    /* 禁用可能导致重绘的CSS属性动画 */
    backface-visibility: hidden;
    perspective: 1000px;
    /* 禁用可能导致窗口圆角问题的效果 */
    will-change: auto;
  }

  /* 针对Windows圆角问题的特殊优化 */
  :global(body) {
    /* 减少GPU层的创建，避免DWM重绘问题 */
    transform: translateZ(0);
    -webkit-font-smoothing: antialiased;
    -moz-osx-font-smoothing: grayscale;
  }

  .innovative-interface {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: transparent;
    overflow: hidden;
  }

  /* 主布局容器 */
  .main-layout {
    display: flex;
    flex: 1;
    overflow: hidden;
    min-height: 0;
    position: relative;
  }

  /* 顶部工具栏 */
  .toolbar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 8px 12px 8px 20px;
    background: linear-gradient(135deg, var(--theme-primary) 0%, var(--theme-secondary) 100%);
    border-bottom: none;
    box-sizing: border-box;
    position: relative;
    overflow: hidden;
    -webkit-app-region: drag;
    min-height: 48px;
  }

  /* macOS风格时调整工具栏布局 */
  .toolbar:has(.mac-controls-container) {
    padding-left: 70px; /* 为左侧按钮留出空间 */
  }

  .toolbar::after {
    content: '';
    position: absolute;
    bottom: 0;
    left: -50px;
    right: -50px;
    height: 30px;
    background: url("data:image/svg+xml,%3Csvg width='120' height='30' viewBox='0 0 120 30' xmlns='http://www.w3.org/2000/svg'%3E%3Cpath d='M0 15 Q30 8 60 15 T120 15 V30 H0 Z' fill='%23ffffff' fill-opacity='0.2'/%3E%3C/svg%3E") repeat-x;
    /* 禁用波浪动画，避免影响窗口圆角 */
    /* animation: wave 8s ease-in-out infinite; */
  }

  @keyframes wave {
    0%, 100% {
      transform: translateX(0);
    }
    25% {
      transform: translateX(-20px);
    }
    50% {
      transform: translateX(-40px);
    }
    75% {
      transform: translateX(-20px);
    }
  }

  .toolbar-left {
    display: flex;
    align-items: center;
    position: relative;
    z-index: 1;
  }

  .toolbar-right {
    display: flex;
    align-items: center;
    gap: 8px;
    position: relative;
    z-index: 1;
    -webkit-app-region: no-drag;
  }

  /* 应用标题 */
  .app-title {
    display: flex;
    align-items: center;
    gap: 8px;
    color: rgba(255, 255, 255, 0.9);
    font-weight: 600;
    font-size: 16px;
  }

  .app-title svg {
    color: rgba(255, 255, 255, 0.8);
  }

  /* 专用拖拽区域 */
  .drag-region {
    flex: 1;
    height: 100%;
    min-width: 100px;
    cursor: move;
    -webkit-app-region: drag;
  }

  /* 主题切换器 */
  .theme-switcher {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px;
    background: rgba(255, 255, 255, 0.1);
    border-radius: 20px;
    backdrop-filter: blur(10px);
    border: 1px solid rgba(255, 255, 255, 0.15);
    -webkit-app-region: no-drag;
  }

  .theme-btn {
    width: 20px;
    height: 20px;
    border-radius: 50%;
    border: 2px solid rgba(255, 255, 255, 0.3);
    cursor: pointer;
    transition: all 0.3s ease;
    position: relative;
    overflow: hidden;
  }

  .theme-btn:hover {
    transform: scale(1.1);
    border-color: rgba(255, 255, 255, 0.6);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.2);
  }

  .theme-btn.active {
    border-color: rgba(255, 255, 255, 0.8);
    box-shadow: 0 0 0 2px rgba(255, 255, 255, 0.4);
    transform: scale(1.05);
  }

  .theme-btn.active::after {
    content: '';
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: 6px;
    height: 6px;
    background: rgba(255, 255, 255, 0.9);
    border-radius: 50%;
    box-shadow: 0 0 4px rgba(0, 0, 0, 0.3);
  }

  /* 搜索容器 */
  .search-container {
    display: flex;
    align-items: center;
    background: rgba(255, 255, 255, 0.15);
    border-radius: 16px;
    padding: 2px;
    backdrop-filter: blur(10px);
    border: 1px solid rgba(255, 255, 255, 0.2);
    transition: all 0.3s ease;
    max-width: 240px;
    height: 32px;
    -webkit-app-region: no-drag;
  }

  .search-container:hover {
    background: rgba(255, 255, 255, 0.2);
    border-color: rgba(255, 255, 255, 0.3);
  }

  .search-container:focus-within {
    background: rgba(255, 255, 255, 0.25);
    border-color: rgba(255, 255, 255, 0.4);
    box-shadow: 0 2px 12px rgba(0, 0, 0, 0.1);
  }

  .search-icon {
    margin-left: 8px;
    color: rgba(255, 255, 255, 0.8);
    flex-shrink: 0;
  }

  .search-input {
    flex: 1;
    padding: 4px 6px;
    border: none;
    background: transparent;
    color: white;
    font-size: 12px;
    outline: none;
    min-width: 0;
  }

  .search-input::placeholder {
    color: rgba(255, 255, 255, 0.7);
  }

  .search-filter-btn {
    background: rgba(255, 255, 255, 0.1);
    border: none;
    border-radius: 50%;
    width: 24px;
    height: 24px;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    margin-right: 2px;
    transition: all 0.2s ease;
    color: rgba(255, 255, 255, 0.8);
  }

  .search-filter-btn:hover {
    background: rgba(255, 255, 255, 0.2);
    color: white;
  }

  /* 操作按钮组 */
  .action-buttons {
    display: flex;
    gap: 6px;
    align-items: center;
    -webkit-app-region: no-drag;
  }

  .toolbar-btn {
    background: rgba(255, 255, 255, 0.15);
    border: 1px solid rgba(255, 255, 255, 0.2);
    border-radius: 18px;
    min-width: 36px;
    width: auto;
    height: 36px;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: all 0.2s ease;
    color: rgba(255, 255, 255, 0.9);
    backdrop-filter: blur(10px);
    padding: 0 8px;
  }

  .toolbar-btn:hover {
    background: rgba(255, 255, 255, 0.25);
    border-color: rgba(255, 255, 255, 0.3);
    color: white;
    /* 禁用transform动画，避免影响窗口圆角 */
    /* transform: translateY(-1px); */
  }

  .toolbar-btn svg {
    width: 14px;
    height: 14px;
  }

  /* 实时监听按钮样式 */
  .realtime-btn {
    position: relative;
  }

  .realtime-btn svg {
    transition: all 0.3s ease;
    color: rgba(255, 255, 255, 0.5); /* 默认状态：半透明白色 */
  }

  .realtime-btn.active svg {
    color: #4CAF50; /* 监听状态：绿色 */
    filter: drop-shadow(0 0 4px rgba(76, 175, 80, 0.6));
  }

  .realtime-btn:hover svg {
    color: rgba(255, 255, 255, 0.8);
  }

  .realtime-btn.active:hover svg {
    color: #66BB6A;
    filter: drop-shadow(0 0 6px rgba(76, 175, 80, 0.8));
  }

  .auto-sync-btn.active {
    background: rgba(76, 175, 80, 0.3);
    border-color: rgba(76, 175, 80, 0.5);
    color: #4CAF50;
  }

  .auto-sync-btn.active:hover {
    background: rgba(76, 175, 80, 0.4);
    border-color: rgba(76, 175, 80, 0.6);
  }

  .countdown-number {
    font-size: 14px;
    font-weight: 600;
    line-height: 1;
    color: inherit;
    min-width: 20px;
    text-align: center;
  }



  .sync-off-text {
    font-size: 12px;
    font-weight: 500;
    line-height: 1;
    color: inherit;
    white-space: nowrap;
    min-width: 30px;
    text-align: center;
  }

  .add-account-btn {
    background: rgba(255, 255, 255, 0.2);
    border-color: rgba(255, 255, 255, 0.3);
  }

  .add-account-btn:hover {
    background: rgba(255, 255, 255, 0.3);
    border-color: rgba(255, 255, 255, 0.4);
  }

  /* 视图切换器 */
  .view-switcher {
    position: relative;
    display: flex;
    align-items: center;
    background: rgba(255, 255, 255, 0.15);
    border-radius: 18px;
    padding: 2px;
    backdrop-filter: blur(10px);
    border: 1px solid rgba(255, 255, 255, 0.2);
    transition: all 0.3s ease;
    height: 36px;
    box-sizing: border-box;
  }

  .view-switcher:hover {
    background: rgba(255, 255, 255, 0.2);
    border-color: rgba(255, 255, 255, 0.3);
  }

  .switcher-track {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    border-radius: 19px;
    overflow: hidden;
    pointer-events: none;
  }

  .switcher-thumb {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 30px;
    height: 30px;
    background: rgba(255, 255, 255, 0.9);
    border-radius: 15px;
    transition: transform 0.4s cubic-bezier(0.25, 0.46, 0.45, 0.94);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.15);
    backdrop-filter: blur(10px);
  }

  .switcher-thumb.pos-0 {
    transform: translateX(1px);
  }

  .switcher-thumb.pos-1 {
    transform: translateX(35px);
  }

  .switcher-thumb.pos-2 {
    transform: translateX(69px);
  }

  .switch-btn {
    position: relative;
    z-index: 2;
    width: 32px;
    height: 32px;
    border: none;
    background: transparent;
    border-radius: 16px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.3s ease;
    color: rgba(255, 255, 255, 0.7);
    margin: 1px;
  }

  .switch-btn.active {
    color: var(--theme-primary);
  }

  .switch-btn:hover {
    color: rgba(255, 255, 255, 0.9);
  }

  .switch-btn.active:hover {
    color: var(--theme-hover);
  }

  .switch-btn svg {
    width: 14px;
    height: 14px;
    stroke-width: 2.5;
  }

  /* 窗口控制按钮容器 */
  .window-controls {
    display: flex;
    align-items: center;
    height: 100%;
    -webkit-app-region: no-drag;
  }

  /* Windows风格 */
  .window-controls.windows-style {
    margin-left: 12px;
  }

  /* macOS控制按钮容器 */
  .mac-controls-container {
    position: absolute;
    left: 10px;
    top: 50%;
    transform: translateY(-50%);
    z-index: 10;
    -webkit-app-region: no-drag;
  }

  /* macOS风格按钮容器 */
  .window-controls.mac-style {
    margin: 0;
    position: static;
  }

  /* macOS风格时右侧标题 */
  .app-title.mac-title-right {
    display: flex;
    align-items: center;
    gap: 8px;
    color: rgba(255, 255, 255, 0.9);
    font-weight: 600;
    font-size: 0.95rem;
    margin-right: 12px;
    -webkit-app-region: drag;
  }

  .app-title.mac-title-right svg {
    flex-shrink: 0;
  }

  .window-control-btn {
    width: 46px;
    height: 32px;
    border: none;
    background: transparent !important;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    color: rgba(255, 255, 255, 0.9) !important;
    outline: none;
    -webkit-app-region: no-drag;
    transition: none !important;
    transform: none !important;
    box-shadow: none !important;
  }

  .window-control-btn svg {
    flex-shrink: 0;
    display: block;
    margin: auto;
  }

  /* Windows风格悬浮效果 */
  .window-controls.windows-style .window-control-btn:hover {
    background: transparent !important;
    color: rgba(255, 255, 255, 0.9) !important;
    transform: none !important;
    transition: none !important;
    box-shadow: none !important;
    border: none !important;
  }

  /* macOS风格按钮 */
  .mac-style .window-control-btn {
    width: 16px;
    height: 16px;
    border-radius: 50%;
    margin-right: 6px;
    position: relative;
    transition: all 0.2s ease !important;
    transform: none !important;
  }

  .mac-style .window-control-btn:last-child {
    margin-right: 0;
  }

  /* macOS按钮颜色 */
  .mac-close {
    background: #ff5f57 !important;
    color: rgba(0, 0, 0, 0.6) !important;
  }

  .mac-minimize {
    background: #ffbd2e !important;
    color: rgba(0, 0, 0, 0.6) !important;
  }

  .mac-maximize {
    background: #28ca42 !important;
    color: rgba(0, 0, 0, 0.6) !important;
  }

  /* macOS按钮悬浮效果 */
  .mac-style .window-control-btn:hover {
    transform: scale(1.1) !important;
    filter: brightness(1.1);
    background: inherit !important;
  }

  /* macOS按钮图标默认隐藏 */
  .mac-style .window-control-btn svg {
    opacity: 0;
    transition: opacity 0.2s ease;
  }

  /* 悬浮时显示图标 */
  .mac-style .window-control-btn:hover svg {
    opacity: 1;
  }



  /* 左侧账号侧边栏 */
  .accounts-sidebar {
    width: 280px;
    background: linear-gradient(180deg, #f8f9fa 0%, #e9ecef 100%);
    border-right: 1px solid #dee2e6;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .sidebar-header {
    padding: 12px 20px;
    border-bottom: 1px solid #dee2e6;
    background: rgba(255, 255, 255, 0.5);
    height: 48px;
    box-sizing: border-box;
    display: flex;
    align-items: center;
  }

  /* 侧边栏切换标签 */
  .sidebar-tabs {
    display: flex;
    background: rgba(255, 255, 255, 0.8);
    border-radius: 8px;
    padding: 3px;
    gap: 2px;
    height: 40px;
    box-sizing: border-box;
    margin: 0 auto;
    justify-content: center;
  }

  .sidebar-tab {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 6px 12px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: #6b7280;
    font-size: 0.85rem;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.2s ease;
    white-space: nowrap;
    min-height: 32px;
    max-height: 32px;
  }

  .sidebar-tab:hover {
    background: rgba(255, 255, 255, 0.6);
    color: #374151;
  }

  .sidebar-tab.active {
    background: linear-gradient(135deg, var(--theme-primary), var(--theme-secondary));
    color: white;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
  }

  .sidebar-tab svg {
    width: 14px;
    height: 14px;
    flex-shrink: 0;
  }

  /* 联系人功能样式 */
  .contacts-section {
    padding: 16px 0;
  }

  .contacts-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 16px;
    padding: 0 16px;
  }

  .contacts-header h4 {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
    color: #374151;
  }

  .add-contact-btn {
    width: 28px;
    height: 28px;
    border: none;
    border-radius: 6px;
    background: linear-gradient(135deg, var(--theme-primary), var(--theme-secondary));
    color: white;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.2s ease;
  }

  .add-contact-btn:hover {
    transform: scale(1.05);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
  }



  .no-contacts {
    text-align: center;
    padding: 32px 16px;
    color: #6b7280;
  }

  .no-contacts-icon {
    margin-bottom: 16px;
    opacity: 1;
    display: flex;
    justify-content: center;
    align-items: center;
  }

  .no-contacts-icon img {
    width: 96px;
    height: 96px;
  }

  .no-contacts-title {
    font-size: 16px;
    font-weight: 600;
    margin: 0 0 8px 0;
    color: #374151;
  }

  .no-contacts-desc {
    font-size: 13px;
    margin: 0;
    line-height: 1.4;
  }

  .accounts-list {
    flex: 1;
    overflow-y: auto;
    padding: 16px 10px 16px 16px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    scrollbar-gutter: stable;
  }

  /* 自定义滚动条样式 */
  .accounts-list::-webkit-scrollbar {
    width: 6px;
  }

  .accounts-list::-webkit-scrollbar-track {
    background: transparent;
  }

  .accounts-list::-webkit-scrollbar-thumb {
    background: rgba(0, 0, 0, 0.2);
    border-radius: 3px;
  }

  .accounts-list::-webkit-scrollbar-thumb:hover {
    background: rgba(0, 0, 0, 0.3);
  }

  .accounts-list::-webkit-scrollbar-corner {
    background: transparent;
  }

  /* 账号分组 */
  .account-group {
    display: flex;
    flex-direction: column;
  }

  .group-header {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 16px;
    background: rgba(255, 255, 255, 0.9);
    border: none;
    border-radius: 10px;
    cursor: pointer;
    transition: all 0.2s ease;
    width: 100%;
    text-align: left;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
    backdrop-filter: blur(10px);
  }

  .group-header:hover {
    background: rgba(255, 255, 255, 0.95);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
  }

  .group-icon {
    width: 32px;
    height: 32px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    background: #ffffff;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
    overflow: hidden;
  }

  .provider-icon {
    width: 24px;
    height: 24px;
    object-fit: contain;
  }

  .group-info {
    flex: 1;
    min-width: 0;
  }

  .group-name {
    font-weight: 600;
    color: #212529;
    margin-bottom: 2px;
    font-size: 0.9rem;
  }

  .group-summary {
    font-size: 0.75rem;
    color: #6c757d;
  }

  .group-toggle {
    transition: transform 0.3s ease;
    color: #6c757d;
  }

  .group-toggle.expanded {
    transform: rotate(180deg);
  }

  .group-accounts {
    margin-top: 8px;
    margin-left: 8px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    border-left: 3px solid rgba(108, 122, 224, 0.3);
    padding-left: 12px;
    background: rgba(255, 255, 255, 0.1);
    border-radius: 0 8px 8px 0;
    padding: 8px 12px 8px 12px;
  }

  .account-item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    background: rgba(255, 255, 255, 0.85);
    border-radius: 8px;
    cursor: pointer;
    transition: all 0.2s ease;
    border: 1px solid rgba(108, 122, 224, 0.15);
    position: relative;
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.08);
    backdrop-filter: blur(8px);
  }

  .account-item:hover {
    background: rgba(255, 255, 255, 0.95);
    border-color: rgba(108, 122, 224, 0.3);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.12);
  }

  .account-item.active {
    background: rgba(255, 255, 255, 0.98);
    border-color: #6c7ae0;
    box-shadow: 0 4px 16px rgba(108, 122, 224, 0.25);
    transform: translateX(2px);
  }

  .account-item.active::before {
    content: '';
    position: absolute;
    left: 0;
    top: 50%;
    transform: translateY(-50%);
    width: 3px;
    height: 50%;
    background: linear-gradient(180deg, var(--theme-primary), var(--theme-secondary));
    border-radius: 0 2px 2px 0;
  }





  .account-info {
    flex: 1;
    min-width: 0;
    position: relative;
  }

  .account-name {
    display: block;
    font-weight: 600;
    font-size: 14px;
    color: #212529;
    margin-bottom: 2px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .account-email {
    display: block;
    font-size: 12px;
    color: #6c757d;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }



  /* 没有账户时的邮箱提供商选项 */
  .no-accounts {
    color: #6c757d;
    font-size: 14px;
    padding: 20px 16px;
  }

  .no-accounts-title {
    text-align: center;
    margin: 0 0 20px 0;
    font-weight: 600;
    color: #495057;
    font-size: 15px;
  }

  .provider-list {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin-bottom: 20px;
  }

  .provider-item {
    width: 100%;
    display: flex;
    align-items: center;
    padding: 14px 16px;
    border: 1px solid #e9ecef;
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.8);
    cursor: pointer;
    transition: all 0.3s ease;
    text-align: left;
    position: relative;
    overflow: hidden;
  }

  .provider-item::before {
    content: '';
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: linear-gradient(135deg, rgba(108, 122, 224, 0.05), rgba(123, 104, 238, 0.05));
    opacity: 0;
    transition: opacity 0.3s ease;
  }

  .provider-item:hover {
    background: rgba(255, 255, 255, 0.95);
    border-color: #6c7ae0;
    transform: translateY(-2px);
    box-shadow: 0 4px 12px rgba(108, 122, 224, 0.15);
  }

  .provider-item:hover::before {
    opacity: 1;
  }

  .provider-icon-wrapper {
    width: 32px;
    height: 32px;
    margin-right: 14px;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    background: rgba(255, 255, 255, 0.9);
    border-radius: 8px;
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.1);
    position: relative;
    z-index: 1;
  }

  .provider-icon-img {
    width: 24px;
    height: 24px;
    object-fit: contain;
  }

  .provider-name {
    flex: 1;
    font-size: 15px;
    color: #495057;
    font-weight: 600;
    position: relative;
    z-index: 1;
  }

  .add-icon {
    color: #6c7ae0;
    font-size: 20px;
    font-weight: bold;
    position: relative;
    z-index: 1;
    transition: transform 0.2s ease;
  }

  .provider-item:hover .add-icon {
    transform: scale(1.1);
  }

  .manual-add {
    text-align: center;
    padding-top: 16px;
    border-top: 1px solid rgba(233, 236, 239, 0.6);
  }

  .manual-add-btn {
    padding: 10px 20px;
    background: rgba(255, 255, 255, 0.8);
    color: #6c757d;
    border: 1px solid #e9ecef;
    border-radius: 8px;
    cursor: pointer;
    font-size: 13px;
    font-weight: 500;
    transition: all 0.3s ease;
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }

  .manual-add-btn:hover {
    background: rgba(255, 255, 255, 0.95);
    color: #495057;
    border-color: #6c7ae0;
    transform: translateY(-1px);
    box-shadow: 0 2px 8px rgba(108, 122, 224, 0.1);
  }

  /* 主内容包装器 */
  .main-content-wrapper {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    transition: all 0.3s ease;
    position: relative;
  }

  /* 设置页面时占满整个宽度 */
  .main-content-wrapper.full-width {
    width: 100vw;
    margin-left: 0;
  }



  /* 主内容区域 */
  .main-content {
    flex: 1;
    overflow: auto;
    background: transparent;
    display: flex;
    flex-direction: column;
    min-height: 0;
    position: relative;
  }



  /* 邮件列表头部 */
  .email-list-header {
    background: #ffffff;
    border-bottom: 1px solid #e9ecef;
    padding: 0;
  }

  .header-top {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 8px 20px;
    height: 48px;
    box-sizing: border-box;
  }

  .header-info {
    flex: 1;
    display: flex;
    align-items: center;
  }

  .account-title {
    font-size: 1rem;
    font-weight: 600;
    color: #212529;
    margin: 0 0 2px 0;
    line-height: 1.2;
  }

  .account-title-container {
    display: flex;
    align-items: center;
    gap: 12px;
    flex: 1;
    height: 32px;
  }

  .inline-stats {
    margin: 0 !important;
  }

  .account-selection-tag {
    background: linear-gradient(135deg, var(--theme-primary), var(--theme-secondary));
    color: white;
    padding: 8px 16px;
    border-radius: 20px;
    font-size: 0.85rem;
    font-weight: 600;
    letter-spacing: 0.3px;
    box-shadow: 0 2px 8px rgba(108, 122, 224, 0.3);
    display: inline-flex;
    align-items: center;
    gap: 6px;
    position: relative;
    overflow: hidden;
    max-width: 100%;
    white-space: nowrap;
  }

  .account-selection-tag::before {
    content: '';
    background-image: url('/youjj.png');
    background-size: contain;
    background-repeat: no-repeat;
    background-position: center;
    width: 16px;
    height: 16px;
    display: inline-block;
    transform: translateY(-1px);
  }

  .tag-text {
    transform: translateY(-1px);
    display: inline-block;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 100%;
  }

  .account-selection-tag::after {
    content: '';
    position: absolute;
    top: 0;
    left: -100%;
    width: 100%;
    height: 100%;
    background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.2), transparent);
    animation: shimmer 2s infinite;
  }

  @keyframes shimmer {
    0% {
      left: -100%;
    }
    100% {
      left: 100%;
    }
  }

  .email-stats {
    display: flex;
    gap: 16px;
    font-size: 0.75rem;
    color: #6c757d;
    line-height: 1.1;
  }

  .unread-count-text {
    color: #dc3545;
    font-weight: 500;
  }

  /* 文件夹工具栏展开/收起按钮 - 与写邮件按钮样式一致 */
  .folder-toggle-btn {
    background: linear-gradient(135deg, var(--theme-primary), var(--theme-secondary));
    color: white;
    border: none;
    padding: 6px 12px;
    border-radius: 6px;
    cursor: pointer;
    font-size: 0.85rem;
    font-weight: 500;
    display: flex;
    align-items: center;
    gap: 6px;
    transition: background 0.2s ease;
    margin-left: auto;
    margin-right: 8px;
    height: 32px;
    box-sizing: border-box;
  }

  .folder-toggle-btn:hover:not(.disabled) {
    background: linear-gradient(135deg, var(--theme-hover), var(--theme-hover-secondary));
  }

  .folder-toggle-btn.disabled {
    background: linear-gradient(135deg, #adb5bd, #6c757d);
    cursor: not-allowed;
    opacity: 0.6;
  }

  .folder-toggle-btn.disabled:hover {
    background: linear-gradient(135deg, #adb5bd, #6c757d);
  }

  .folder-icon {
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .folder-toggle-text {
    font-size: 0.85rem;
    font-weight: 500;
    line-height: 1;
    transform: translateY(1px);
  }

  .toggle-arrow {
    display: flex;
    align-items: center;
    justify-content: center;
    transition: transform 0.2s ease;
  }

  .toggle-arrow.rotated {
    transform: rotate(180deg);
  }

  .header-actions {
    display: flex;
    gap: 24px;
    align-items: center;
  }

  /* 写邮件按钮 */
  .compose-btn {
    background: linear-gradient(135deg, var(--theme-primary), var(--theme-secondary));
    color: white;
    border: none;
    padding: 6px 12px;
    border-radius: 6px;
    cursor: pointer;
    font-size: 0.85rem;
    font-weight: 500;
    display: flex;
    align-items: center;
    gap: 6px;
    transition: background 0.2s ease;
    height: 32px;
    box-sizing: border-box;
  }

  .compose-btn:hover {
    background: linear-gradient(135deg, var(--theme-hover), var(--theme-hover-secondary));
  }

  /* 悬浮文件夹工具栏 */
  .floating-folder-toolbar {
    position: absolute;
    bottom: 20px;
    left: 50%;
    transform: translateX(-50%);
    z-index: 1000;
    animation: floatingSlideUp 0.3s cubic-bezier(0.25, 0.46, 0.45, 0.94);
    pointer-events: none; /* 让容器本身不阻挡点击 */
  }

  .floating-folder-toolbar .email-folders {
    pointer-events: auto; /* 恢复按钮的点击功能 */
  }

  @keyframes floatingSlideUp {
    from {
      transform: translateX(-50%) translateY(100%);
      opacity: 0;
    }
    to {
      transform: translateX(-50%) translateY(0);
      opacity: 1;
    }
  }

  @keyframes slideInUp {
    from {
      transform: translateY(100%);
      opacity: 0;
    }
    to {
      transform: translateY(0);
      opacity: 1;
    }
  }

  /* 邮件文件夹导航 */
  .email-folders {
    display: flex;
    gap: 8px;
    background: linear-gradient(135deg, #1a1a1a 0%, #2d2d2d 100%);
    padding: 8px;
    border-radius: 12px;
    backdrop-filter: blur(10px);
    border: 2px solid #ffffff;
    box-shadow:
      0 4px 16px rgba(0, 0, 0, 0.4),
      0 0 10px rgba(255, 255, 255, 0.5),
      0 0 20px rgba(255, 255, 255, 0.3),
      0 0 30px rgba(255, 255, 255, 0.1);
  }

  .folder-btn {
    background: transparent;
    border: none;
    padding: 8px 16px;
    border-radius: 8px;
    cursor: pointer;
    font-size: 0.8rem;
    color: rgba(255, 255, 255, 0.7);
    display: flex;
    align-items: center;
    gap: 6px;
    transition: all 0.2s ease;
    white-space: nowrap;
    font-weight: 500;
  }

  .folder-btn:hover {
    background: rgba(108, 122, 224, 0.2);
    color: rgba(255, 255, 255, 0.9);
  }

  .folder-btn.active {
    background: linear-gradient(135deg, var(--theme-primary), var(--theme-secondary));
    color: white;
    box-shadow: 0 2px 8px rgba(108, 122, 224, 0.5);
  }

  /* 邮件详情底部弹窗 */
  .email-detail-sidebar {
    position: fixed;
    background: linear-gradient(135deg, #ffffff 0%, #f8f9fa 100%);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    box-shadow: 0 -4px 20px rgba(0, 0, 0, 0.15);
    z-index: 1000;
  }

  /* 从下往上滑入的样式 */
  .email-detail-sidebar.slide-up {
    bottom: 0;
    left: 280px; /* 左侧账号栏的宽度 */
    right: 0;
    height: 75vh;
    border-top: 1px solid #e9ecef;
    border-radius: 20px 20px 0 0;
    animation: slideInUp 0.4s cubic-bezier(0.25, 0.46, 0.45, 0.94);
  }

  /* 从左往右滑入的样式 */
  .email-detail-sidebar.slide-right {
    top: 0;
    left: 0;
    bottom: 0;
    width: 60vw;
    border-right: 1px solid #e9ecef;
    border-radius: 0 20px 20px 0;
    animation: slideInRight 0.4s cubic-bezier(0.25, 0.46, 0.45, 0.94);
  }

  /* 从右往左滑入的样式 */
  .email-detail-sidebar.slide-left {
    top: 0;
    right: 0;
    bottom: 0;
    width: 60vw;
    border-left: 1px solid #e9ecef;
    border-radius: 20px 0 0 20px;
    animation: slideInLeft 0.4s cubic-bezier(0.25, 0.46, 0.45, 0.94);
  }

  @keyframes slideInUp {
    from {
      transform: translateY(100%);
    }
    to {
      transform: translateY(0);
    }
  }

  @keyframes slideInRight {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(0);
    }
  }

  @keyframes slideInLeft {
    from {
      transform: translateX(100%);
    }
    to {
      transform: translateX(0);
    }
  }

  /* 新的邮件头部样式 */
  .email-header {
    padding: 24px;
    background: linear-gradient(135deg, #ffffff 0%, #f8f9fa 100%);
    border-bottom: 1px solid rgba(0, 0, 0, 0.08);
    border-radius: 20px 20px 0 0;
  }

  .email-meta {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    margin-bottom: 16px;
  }

  .sender-info {
    display: flex;
    align-items: center;
    gap: 16px;
    flex: 1;
  }

  .sender-avatar {
    width: 48px;
    height: 48px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 18px;
    font-weight: 600;
    color: white;
    flex-shrink: 0;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
  }

  .sender-avatar img {
    width: 100%;
    height: 100%;
    border-radius: 50%;
    object-fit: cover;
  }

  .sender-details {
    flex: 1;
    min-width: 0;
  }

  .sender-name {
    font-size: 18px;
    font-weight: 600;
    color: #1f2937;
    margin: 0 0 4px 0;
    line-height: 1.2;
  }

  .sender-email {
    font-size: 14px;
    color: #6b7280;
    margin: 0 0 4px 0;
    line-height: 1.2;
  }

  .email-time {
    font-size: 13px;
    color: #9ca3af;
    margin: 0;
    line-height: 1.2;
  }

  .email-actions {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .email-action-btn {
    width: 40px;
    height: 40px;
    border: none;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.8);
    color: #6b7280;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.2s ease;
    backdrop-filter: blur(10px);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
  }

  .email-action-btn svg {
    width: 18px;
    height: 18px;
    stroke: currentColor;
    fill: none;
  }

  .email-action-btn:hover {
    background: linear-gradient(135deg, var(--theme-primary), var(--theme-secondary));
    color: white;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.2);
  }

  /* 翻译按钮的特殊样式 */
  .translate-btn:hover {
    background: linear-gradient(135deg, #3b82f6, #1d4ed8) !important;
    color: white !important;
  }

  .translate-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  /* 翻译进度动画 */
  .animate-spin {
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

  /* 翻译进度提示 */
  .translation-progress {
    position: absolute;
    top: 50px;
    right: 20px;
    background: #3b82f6;
    color: #ffffff !important;
    padding: 12px 18px;
    border-radius: 8px;
    font-size: 14px;
    font-weight: 600;
    box-shadow: 0 4px 12px rgba(59, 130, 246, 0.4);
    z-index: 1000;
    animation: fadeInOut 0.3s ease-in-out;
    border: 1px solid rgba(255, 255, 255, 0.2);
    backdrop-filter: blur(10px);
    display: flex;
    align-items: center;
    gap: 10px;
    white-space: nowrap;
  }

  .progress-spinner {
    width: 16px;
    height: 16px;
    border: 2px solid rgba(255, 255, 255, 0.3);
    border-top: 2px solid #ffffff;
    border-radius: 50%;
    animation: spin 1s linear infinite;
    flex-shrink: 0;
  }

  .translation-progress-text {
    color: #ffffff;
    font-weight: 600;
    font-size: 14px;
    line-height: 1;
    margin: 0;
    padding: 0;
  }

  @keyframes fadeInOut {
    0% {
      opacity: 0;
      transform: translateY(-10px);
    }
    100% {
      opacity: 1;
      transform: translateY(0);
    }
  }

  /* 邮件详情关闭按钮的特殊悬停效果 */
  .email-actions .email-action-btn:last-child:hover {
    background: #ef4444;
    color: white;
  }



  .email-subject {
    margin-top: 8px;
  }

  .email-subject h2 {
    font-size: 24px;
    font-weight: 700;
    color: #111827;
    margin: 0;
    line-height: 1.3;
  }

  /* 次要操作按钮 */
  .secondary-actions {
    padding: 12px 24px;
    background: rgba(248, 250, 252, 0.8);
    border-bottom: 1px solid rgba(0, 0, 0, 0.06);
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }

  .secondary-btn {
    padding: 6px 12px;
    border: 1px solid #e5e7eb;
    border-radius: 20px;
    background: white;
    color: #6b7280;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 6px;
    transition: all 0.2s ease;
  }

  .secondary-btn:hover {
    background: #f3f4f6;
    border-color: #d1d5db;
    color: #374151;
  }

  .secondary-btn.read-btn,
  .secondary-btn.not-spam-btn {
    border-color: #10b981;
    color: #10b981;
  }

  .secondary-btn.read-btn:hover,
  .secondary-btn.not-spam-btn:hover {
    background: #10b981;
    color: white;
  }

  .secondary-btn.unread-btn {
    border-color: #6b7280;
    color: #6b7280;
  }

  .secondary-btn.unread-btn:hover {
    background: #6b7280;
    color: white;
  }

  .secondary-btn.spam-btn {
    border-color: #ef4444;
    color: #ef4444;
  }

  .secondary-btn.spam-btn:hover {
    background: #ef4444;
    color: white;
  }

  /* 邮件内容区域 */
  .email-content {
    flex: 1;
    padding: 24px;
    overflow-y: auto;
    background: white;
    /* 自定义滚动条样式 - Firefox */
    scrollbar-width: thin;
    scrollbar-color: rgba(0, 0, 0, 0.2) transparent;
  }

  /* 自定义滚动条样式 - Webkit浏览器 */
  .email-content::-webkit-scrollbar {
    width: 8px;
  }

  .email-content::-webkit-scrollbar-track {
    background: transparent;
    border-radius: 4px;
  }

  .email-content::-webkit-scrollbar-thumb {
    background: linear-gradient(135deg, var(--theme-primary), var(--theme-secondary));
    border-radius: 4px;
    transition: all 0.2s ease;
    opacity: 0.6;
  }

  .email-content::-webkit-scrollbar-thumb:hover {
    opacity: 0.8;
    background: linear-gradient(135deg, var(--theme-hover), var(--theme-hover-secondary));
  }

  .email-content::-webkit-scrollbar-thumb:active {
    opacity: 1;
    background: linear-gradient(135deg, var(--theme-hover), var(--theme-hover-secondary));
  }

  .email-body {
    font-size: 16px;
    line-height: 1.6;
    color: #374151;
  }

  .text-content {
    white-space: pre-wrap;
    font-family: inherit;
    margin: 0;
  }

  .no-content {
    text-align: center;
    color: #9ca3af;
    font-style: italic;
    padding: 40px 20px;
  }

  /* 内联翻译样式 */
  .translation-indicator {
    display: flex;
    align-items: center;
    justify-content: space-between;
    background: linear-gradient(135deg, #f0f9ff, #e0f2fe);
    border: 1px solid #0ea5e9;
    border-radius: 8px;
    padding: 12px 16px;
    margin-bottom: 16px;
  }

  .translation-status {
    display: flex;
    align-items: center;
    gap: 8px;
    color: #0369a1;
    font-weight: 500;
    font-size: 14px;
  }

  .translation-status svg {
    color: #0ea5e9;
  }

  .translation-text {
    color: #0369a1;
  }

  .show-original-btn {
    background: #0ea5e9;
    color: white;
    border: none;
    border-radius: 6px;
    padding: 6px 12px;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .show-original-btn:hover {
    background: #0284c7;
    transform: translateY(-1px);
  }







  /* 关闭标签样式 */
  .close-label {
    background: rgba(108, 122, 224, 0.1);
    border: none;
    cursor: pointer;
    padding: 6px 12px;
    border-radius: 6px;
    color: var(--theme-primary);
    font-size: 0.85rem;
    font-weight: 500;
    letter-spacing: 0.5px;
    transition: all 0.2s ease;
  }

  .close-label:hover {
    background: linear-gradient(135deg, var(--theme-primary), var(--theme-secondary));
    color: white;
  }





  .detail-avatar {
    width: 40px;
    height: 40px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    color: white;
    font-weight: 600;
    font-size: 16px;
    flex-shrink: 0;
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.1);
    border: 2px solid rgba(255, 255, 255, 0.8);
  }

  .detail-avatar-image {
    width: 100%;
    height: 100%;
    object-fit: cover;
    border-radius: 50%;
  }

  .sender-info {
    flex: 1;
    min-width: 0;
  }

  .detail-from {
    font-weight: 600;
    color: #1a202c;
    font-size: 14px;
    margin-bottom: 3px;
    line-height: 1.3;
  }

  .detail-recipients {
    font-size: 12px;
    color: #64748b;
    margin-top: 0;
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .recipient-label {
    font-weight: 500;
    color: #475569;
  }

  .recipient-value {
    color: #334155;
    font-weight: 500;
  }

  .detail-time {
    color: #64748b;
    font-size: 12px;
    font-weight: 500;
    line-height: 1.3;
    background: rgba(100, 116, 139, 0.08);
    padding: 4px 8px;
    border-radius: 12px;
    white-space: nowrap;
  }

  .detail-subject {
    font-size: 16px;
    font-weight: 600;
    color: #2c3e50;
    margin: 0 0 16px 0;
    line-height: 1.3;
  }

  .detail-body {
    color: #495057;
    line-height: 1.6;
    font-size: 14px;
    margin-bottom: 20px;
    /* 移除flex相关样式，让内容自然流动 */
  }



  .no-content-detail {
    text-align: center;
    color: #6c757d;
    padding: 40px 20px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
  }

  .no-content-detail .no-content-icon {
    font-size: 3rem;
    margin-bottom: 16px;
  }

  .no-content-detail h4 {
    margin: 0 0 8px 0;
    font-size: 1.1rem;
    color: #495057;
  }

  .no-content-detail p {
    margin: 0 0 16px 0;
    font-size: 0.9rem;
  }





  .action-button {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    border: 1px solid #dee2e6;
    border-radius: 6px;
    background: white;
    color: #495057;
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.2s ease;
    width: 100%;
    justify-content: center;
  }

  .action-button:hover {
    background: #f8f9fa;
    border-color: #adb5bd;
  }

  .action-button.primary {
    background: linear-gradient(135deg, var(--theme-primary), var(--theme-secondary));
    color: white;
    border-color: var(--theme-primary);
  }

  .action-button.primary:hover {
    background: linear-gradient(135deg, var(--theme-hover), var(--theme-hover-secondary));
    border-color: var(--theme-hover);
  }

  .action-button.secondary {
    background: linear-gradient(135deg, var(--theme-primary), var(--theme-secondary));
    color: white;
    border-color: var(--theme-primary);
    opacity: 0.8;
  }

  .action-button.secondary:hover {
    background: linear-gradient(135deg, var(--theme-hover), var(--theme-hover-secondary));
    border-color: var(--theme-hover);
    opacity: 1;
  }

  .action-button.danger {
    background: linear-gradient(135deg, #dc3545, #c82333);
    color: white;
    border-color: #dc3545;
  }

  .action-button.danger:hover {
    background: linear-gradient(135deg, #c82333, #bd2130);
    border-color: #c82333;
  }

  .action-button.not-spam-btn {
    background: #28a745 !important;
    color: white !important;
    border: 1px solid #28a745 !important;
  }

  .action-button.not-spam-btn:hover {
    background: #28a745 !important;
    color: white !important;
    border: 1px solid #28a745 !important;
    transform: none !important;
  }

  .action-button.spam-btn {
    background: #dc3545 !important;
    color: white !important;
    border: 1px solid #dc3545 !important;
  }

  .action-button.spam-btn:hover {
    background: #dc3545 !important;
    color: white !important;
    border: 1px solid #dc3545 !important;
    transform: none !important;
  }

  .action-button.read-btn {
    background: #28a745 !important;
    color: white !important;
    border: 1px solid #28a745 !important;
  }

  .action-button.read-btn:hover {
    background: #28a745 !important;
    color: white !important;
    border: 1px solid #28a745 !important;
    transform: none !important;
  }

  .action-button.unread-btn {
    background: #6c757d !important;
    color: white !important;
    border: 1px solid #6c757d !important;
  }

  .action-button.unread-btn:hover {
    background: #6c757d !important;
    color: white !important;
    border: 1px solid #6c757d !important;
    transform: none !important;
  }

  .folder-btn svg {
    flex-shrink: 0;
  }

  .header-btn {
    background: #f8f9fa;
    border: none;
    padding: 8px;
    border-radius: 6px;
    cursor: pointer;
    color: #6c757d;
    transition: all 0.2s ease;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
  }

  .header-btn:hover {
    background: #e9ecef;
    color: #495057;
  }

  .header-btn svg {
    width: 16px;
    height: 16px;
  }

  /* 邮件卡片容器 */
  .email-cards-container {
    padding: 16px 10px 16px 16px;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
    grid-auto-rows: min-content;
    gap: 20px;
    overflow-y: auto;
    overflow-x: hidden;
    height: 100%;
    scrollbar-gutter: stable;
    align-content: start;
    transition: all 0.3s ease;
    position: relative;
    z-index: 1;
  }

  /* 加载状态容器 */
  .loading-container-wrapper {
    grid-column: 1 / -1;
    display: flex;
    justify-content: center;
    align-items: center;
    min-height: 400px;
    width: 100%;
  }

  /* 清空缓存进度条样式 */
  .clear-cache-progress {
    grid-column: 1 / -1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 2rem;
    max-width: 400px;
    margin: 0 auto;
    text-align: center;
    min-height: 400px;
  }

  .progress-header {
    display: flex;
    flex-direction: column;
    align-items: center;
    margin-bottom: 2rem;
  }

  .progress-icon {
    font-size: 3rem;
    margin-bottom: 1rem;
    opacity: 0.8;
  }

  .progress-title {
    font-size: 1.2rem;
    font-weight: 600;
    color: #374151;
    margin-bottom: 0.5rem;
  }

  .progress-bar-container {
    display: flex;
    align-items: center;
    gap: 1rem;
    width: 100%;
    margin-bottom: 1rem;
  }

  .progress-bar {
    flex: 1;
    height: 8px;
    background: #e5e7eb;
    border-radius: 4px;
    overflow: hidden;
  }

  .progress-fill {
    height: 100%;
    background: linear-gradient(90deg, #3b82f6, #1d4ed8);
    border-radius: 4px;
    transition: width 0.3s ease;
  }

  .cache-progress-percentage {
    font-size: 0.9rem;
    font-weight: 600;
    color: #3b82f6;
    min-width: 40px;
  }

  .progress-step {
    font-size: 0.9rem;
    color: #6b7280;
    margin-top: 0.5rem;
    min-height: 1.2rem;
  }

  /* 标题栏样式设置 */
  .titlebar-style-section {
    margin-top: 1rem;
  }

  .style-options {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1rem;
    margin-bottom: 1rem;
  }

  .style-option {
    display: block;
    cursor: pointer;
    border: 2px solid #e5e7eb;
    border-radius: 8px;
    padding: 1rem;
    transition: all 0.2s ease;
    background: white;
  }

  .style-option:hover {
    border-color: #d1d5db;
    /* 移除悬浮效果 */
  }

  .style-option.active {
    border-color: var(--theme-primary);
    background: rgba(var(--theme-primary-rgb), 0.1);
  }

  .style-option input[type="radio"] {
    display: none;
  }

  .option-content {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.75rem;
  }

  .option-preview {
    width: 100%;
    height: 60px;
    border-radius: 6px;
    overflow: hidden;
    border: 1px solid #e5e7eb;
  }

  .preview-titlebar {
    width: 100%;
    height: 100%;
    background: linear-gradient(135deg, var(--theme-primary) 0%, var(--theme-secondary) 100%);
    display: flex;
    align-items: center;
    padding: 0 8px;
    position: relative;
    justify-content: space-between;
    box-sizing: border-box;
  }

  .titlebar-preview .preview-title {
    color: white;
    font-size: 0.7rem;
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    display: flex;
    align-items: center;
    gap: 4px;
    height: 100%;
    line-height: 1;
  }

  .preview-icon {
    flex-shrink: 0;
    color: rgba(255, 255, 255, 0.8);
  }

  .preview-controls {
    display: flex;
    align-items: center;
    gap: 3px;
    flex-shrink: 0;
    height: 100%;
  }

  /* Windows布局：标题在左，按钮在右 */
  .preview-titlebar.windows-layout {
    justify-content: space-between;
  }

  .titlebar-preview .preview-title.windows-title-left {
    order: 1;
    margin-right: auto;
  }

  .titlebar-preview .windows-controls {
    order: 2;
  }

  /* macOS布局：按钮在左，标题在右 */
  .preview-titlebar.mac-layout {
    justify-content: space-between;
  }

  .titlebar-preview .mac-controls {
    order: 1;
  }

  .titlebar-preview .preview-title.mac-title-right {
    order: 2;
    margin-left: auto;
  }



  .control-btn {
    width: 10px;
    height: 10px;
    border-radius: 2px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 7px;
    color: rgba(255, 255, 255, 0.8);
    background: rgba(255, 255, 255, 0.2);
    flex-shrink: 0;
    line-height: 1;
  }

  /* macOS预览按钮 */
  .mac-close {
    background: #ff5f57 !important;
    border-radius: 50%;
    color: transparent;
    width: 10px;
    height: 10px;
  }

  .mac-minimize {
    background: #ffbd2e !important;
    border-radius: 50%;
    color: transparent;
    width: 10px;
    height: 10px;
  }

  .mac-maximize {
    background: #28ca42 !important;
    border-radius: 50%;
    color: transparent;
    width: 10px;
    height: 10px;
  }

  .option-info {
    text-align: center;
  }

  .option-title {
    font-weight: 600;
    color: #374151;
    margin-bottom: 0.25rem;
  }

  .option-desc {
    font-size: 0.875rem;
    color: #6b7280;
    text-align: center;
  }

  .titlebar-info {
    background: #f8fafc;
    border-radius: 6px;
    padding: 0.75rem;
  }

  /* 空状态 */
  .empty-message {
    grid-column: 1 / -1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 60px 20px;
    color: #6c757d;
    text-align: center;
  }

  .empty-icon {
    font-size: 3rem;
    margin-bottom: 16px;
    animation: pulse 2s infinite;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.5; }
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }

  .spinning {
    animation: spin 1s linear infinite;
  }

  /* 未读邮件指示器 */
  .unread-indicator {
    width: 8px;
    height: 8px;
    background: #dc3545;
    border-radius: 50%;
  }

  .star-indicator {
    font-size: 14px;
  }

  /* 邮件状态样式 - 未读邮件增强效果 */
  .email-card.unread {
    background: rgba(255, 255, 255, 0.9);
    box-shadow:
      0 8px 32px rgba(108, 122, 224, 0.15),
      0 2px 8px rgba(0, 0, 0, 0.05),
      inset 0 1px 0 rgba(255, 255, 255, 0.3);
  }

  .email-card.read {
    background: rgba(255, 255, 255, 0.8);
  }

  .email-card.deleted {
    background: rgba(248, 249, 250, 0.7) !important;
    backdrop-filter: blur(10px);
    opacity: 0.6;
    color: #6c757d;
  }

  .email-card.deleted .sender-name,
  .email-card.deleted .email-subject,
  .email-card.deleted .email-preview {
    color: #6c757d;
  }

  .email-card.deleted .sender-avatar {
    background-color: #adb5bd !important;
  }

  .email-card.deleted::before {
    content: "🗑️ 已删除";
    position: absolute;
    top: 8px;
    right: 8px;
    background: rgba(108, 117, 125, 0.1);
    color: #6c757d;
    padding: 2px 6px;
    border-radius: 4px;
    font-size: 0.7rem;
    font-weight: 500;
  }

  /* 标签样式 */
  .folder-tag {
    background: #e9ecef;
    color: #495057;
    padding: 2px 6px;
    border-radius: 4px;
    font-size: 0.7rem;
    text-transform: uppercase;
  }

  .folder-tag.loading {
    background: #f8f9fa;
    color: #6c757d;
    animation: pulse 1.5s ease-in-out infinite;
  }

  .folder-tag.error {
    background: #dc3545;
    color: white;
  }

  /* 智能标签样式 */
  .smart-tag {
    padding: 3px 8px;
    border-radius: 12px;
    font-size: 0.7rem;
    font-weight: 600;
    text-transform: none;
    letter-spacing: 0.3px;
    margin-right: 4px;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
  }

  .smart-tag.verificationcode {
    background: linear-gradient(135deg, #e74c3c, #c0392b) !important;
    color: white !important;
    font-weight: 700;
  }

  .smart-tag.important {
    background: linear-gradient(135deg, #f39c12, #e67e22) !important;
    color: white !important;
    font-weight: 600;
  }

  .smart-tag.folder {
    background: #95a5a6 !important;
    color: white !important;
    font-weight: 500;
  }

  @keyframes pulse {
    0%, 100% {
      opacity: 1;
    }
    50% {
      opacity: 0.5;
    }
  }

  .status-tag.unread {
    background: #dc3545;
    color: white;
    padding: 2px 6px;
    border-radius: 4px;
    font-size: 0.7rem;
    font-weight: 500;
  }

  .status-tag.read {
    background: #28a745;
    color: white;
    padding: 2px 6px;
    border-radius: 4px;
    font-size: 0.7rem;
    font-weight: 500;
  }



  /* 邮件卡片容器滚动条样式 */
  .email-cards-container::-webkit-scrollbar {
    width: 6px;
  }

  .email-cards-container::-webkit-scrollbar-track {
    background: transparent;
  }

  .email-cards-container::-webkit-scrollbar-thumb {
    background: rgba(0, 0, 0, 0.2);
    border-radius: 3px;
  }

  .email-cards-container::-webkit-scrollbar-thumb:hover {
    background: rgba(0, 0, 0, 0.3);
  }

  .email-cards-container::-webkit-scrollbar-corner {
    background: transparent;
  }

  /* 邮件卡片 - 简化效果避免圆角问题 */
  .email-card {
    background: rgba(255, 255, 255, 0.95);
    /* 暂时禁用毛玻璃效果，避免Windows圆角问题 */
    /* backdrop-filter: blur(20px); */
    /* -webkit-backdrop-filter: blur(20px); */
    border-radius: 16px;
    padding: 16px;
    box-shadow:
      0 8px 32px rgba(0, 0, 0, 0.1),
      0 2px 8px rgba(0, 0, 0, 0.05),
      inset 0 1px 0 rgba(255, 255, 255, 0.2);
    border: none;
    cursor: pointer;
    transition: all 0.3s cubic-bezier(0.25, 0.46, 0.45, 0.94);
    display: flex;
    flex-direction: column;
    gap: 12px;
    height: fit-content;
    align-self: start;
    position: relative;
    width: 100%;
    box-sizing: border-box;
    min-width: 0;
  }



  .email-card:hover {
    background: rgba(255, 255, 255, 0.98);
    box-shadow:
      0 12px 40px rgba(0, 0, 0, 0.15),
      0 4px 16px rgba(0, 0, 0, 0.08),
      inset 0 1px 0 rgba(255, 255, 255, 0.3);
    /* 禁用transform动画，避免影响窗口圆角 */
    /* transform: translateY(-2px); */
  }

  /* 卡片头部 */
  .card-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
  }

  .sender-section {
    display: flex;
    align-items: center;
    gap: 10px;
    flex: 1;
    min-width: 0;
    overflow: hidden;
  }

  .card-meta {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 6px;
    flex-shrink: 0;
    margin-left: 12px;
  }

  /* 卡片内容 */
  .card-content {
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  /* 主题和内容行 - 水平布局对齐 */
  .subject-line,
  .content-line {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    min-height: 24px;
  }

  /* 内容标签样式 - 对齐设计 */
  .content-label {
    font-size: 0.7rem;
    font-weight: 600;
    color: var(--theme-primary);
    text-transform: uppercase;
    letter-spacing: 0.3px;
    opacity: 0.8;
    flex-shrink: 0;
    width: 28px;
    line-height: 1.3;
    margin-top: 2px;
    text-align: left;
    display: inline-block;
  }

  .email-subject {
    font-size: 0.95rem;
    font-weight: 600;
    color: #212529;
    margin: 0;
    line-height: 1.3;
    word-wrap: break-word;
    overflow-wrap: break-word;
    flex: 1;
    min-width: 0;
  }

  .email-content-wrapper {
    border: 1px dashed #d0d7de;
    border-radius: 6px;
    padding: 6px 8px;
    background: rgba(248, 249, 250, 0.3);
    border-style: dashed;
    flex: 1;
    min-width: 0;
  }

  .email-preview {
    color: #6c757d;
    font-size: 0.8rem;
    line-height: 1.2;
    margin: 0;
    display: -webkit-box;
    -webkit-line-clamp: 1;
    line-clamp: 1;
    -webkit-box-orient: vertical;
    overflow: hidden;
    max-height: 1.2rem;
    word-wrap: break-word;
    overflow-wrap: break-word;
    max-width: 100%;
  }

  /* 卡片底部 */
  .card-footer {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding-top: 6px;
    border-top: 1px solid #f1f3f4;
  }

  .card-tags {
    display: flex;
    gap: 6px;
    align-items: center;
  }



  .card-actions {
    display: flex;
    gap: 8px;
  }





  .sender-avatar {
    width: 36px;
    height: 36px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 0.9rem;
    color: white;
    font-weight: 600;
    flex-shrink: 0;
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.1);
    overflow: hidden;
  }

  .avatar-image {
    width: 100%;
    height: 100%;
    object-fit: cover;
    border-radius: 50%;
  }

  .timeline-avatar-image {
    width: 100%;
    height: 100%;
    object-fit: cover;
    border-radius: 50%;
  }

  .chat-avatar-image {
    width: 100%;
    height: 100%;
    object-fit: cover;
    border-radius: 50%;
  }

  .detail-avatar-image {
    width: 100%;
    height: 100%;
    object-fit: cover;
    border-radius: 50%;
  }

  .email-card .sender-info {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    display: block;
  }

  .sender-name {
    font-weight: 600;
    color: #212529;
    margin-bottom: 2px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    font-size: 0.95rem;
    max-width: 100%;
  }

  .sender-email {
    font-size: 0.8rem;
    color: #6c757d;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 500px;
  }

  .send-time {
    color: #6c757d;
    font-size: 0.85rem;
    white-space: nowrap;
    font-weight: 500;
    min-width: 120px;
    text-align: right;
  }



  .email-subject {
    font-weight: 500;
    color: #212529;
    margin: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    width: 100%;
  }

  .email-preview {
    color: #6c757d;
    font-size: 0.9rem;
    margin: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    width: 100%;
  }



  .send-time {
    color: #6c757d;
    font-size: 0.85rem;
    white-space: nowrap;
  }



  .card-action-btn {
    background: #f8f9fa;
    border: none;
    padding: 6px;
    border-radius: 6px;
    cursor: pointer;
    color: #6c757d;
    transition: background-color 0.15s ease;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
  }

  .card-action-btn:hover {
    background: #e9ecef;
    color: #495057;
  }

  .card-action-btn svg {
    width: 14px;
    height: 14px;
  }

  .card-action-btn.delete-btn:hover {
    background: #fee;
    color: #dc3545;
  }

  .card-action-btn.restore-btn:hover {
    background: #e8f5e8;
    color: #28a745;
  }

  .card-action-btn.star-btn.starred {
    color: #ffc107;
  }

  .card-action-btn.star-btn.starred:hover {
    background: #fff3cd;
    color: #ffc107;
  }

  .card-action-btn.card-reply-btn:hover {
    background: linear-gradient(135deg, var(--theme-primary), var(--theme-secondary));
    color: white;
  }

  .card-action-btn.star-btn:not(.starred):hover {
    background: #fff3cd;
    color: #ffc107;
  }







  /* 时间线布局 */
  .timeline-container {
    position: relative;
    max-width: 800px;
    margin: 0 auto;
    padding: 20px;
    height: 100%;
  }

  /* 当时间线容器有邮件时，恢复正常布局 */
  .timeline-container:not(:has(.simple-welcome)) {
    display: block;
  }

  /* 当时间线容器只有欢迎界面时，使用居中布局 */
  .timeline-container:has(.simple-welcome) {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
  }

  .timeline-line {
    position: absolute;
    left: 50%;
    top: 0;
    bottom: 0;
    width: 4px;
    background: linear-gradient(to bottom, #3b82f6, #1d4ed8);
    border-radius: 2px;
    transform: translateX(-50%);
  }

  .timeline-item {
    position: relative;
    margin-bottom: 40px;
    width: 45%;
  }

  .timeline-item.left {
    left: 0;
    text-align: right;
  }

  .timeline-item.right {
    left: 55%;
    text-align: left;
  }

  .timeline-marker {
    position: absolute;
    width: 60px;
    height: 60px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 1.5rem;
    color: white;
    border: 4px solid white;
    box-shadow: 0 5px 15px rgba(0, 0, 0, 0.2);
    overflow: hidden;
  }

  .timeline-item.left .timeline-marker {
    right: -85px;
  }

  .timeline-item.right .timeline-marker {
    left: -85px;
  }

  .timeline-content {
    background: white;
    padding: 20px;
    border-radius: 15px;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.1);
    transition: all 0.3s ease;
  }

  .timeline-content:hover {
    transform: translateY(-3px);
    box-shadow: 0 15px 35px rgba(0, 0, 0, 0.15);
  }

  .timeline-item.deleted .timeline-content {
    background: #f8f9fa;
    opacity: 0.6;
    color: #6c757d;
  }

  .timeline-item.deleted .timeline-content h4,
  .timeline-item.deleted .timeline-content p {
    color: #6c757d;
  }

  .timeline-time {
    color: #667eea;
    font-weight: 600;
    margin-bottom: 10px;
  }

  .timeline-content h4 {
    margin: 0 0 10px 0;
    color: #333;
    font-size: 1.1rem;
  }

  .timeline-from {
    color: #666;
    font-size: 0.9rem;
    margin: 0 0 10px 0;
  }

  .timeline-preview {
    color: #555;
    line-height: 1.5;
    margin: 0;
  }

  /* 聊天布局 */
  .chat-container {
    max-width: 700px;
    margin: 0 auto;
    padding: 20px;
    height: 100%;
  }

  /* 当聊天容器有邮件时，恢复正常布局 */
  .chat-container:not(:has(.simple-welcome)) {
    display: block;
  }

  /* 当聊天容器只有欢迎界面时，使用居中布局 */
  .chat-container:has(.simple-welcome) {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
  }

  .chat-message {
    display: flex;
    align-items: flex-start;
    gap: 15px;
    margin-bottom: 25px;
  }

  .message-avatar {
    width: 50px;
    height: 50px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 1.3rem;
    color: white;
    flex-shrink: 0;
    overflow: hidden;
  }

  .message-bubble {
    background: white;
    border-radius: 20px;
    padding: 15px 20px;
    box-shadow: 0 5px 15px rgba(0, 0, 0, 0.1);
    max-width: 80%;
    position: relative;
  }

  .message-bubble::before {
    content: '';
    position: absolute;
    left: -10px;
    top: 20px;
    width: 0;
    height: 0;
    border-style: solid;
    border-width: 10px 10px 10px 0;
    border-color: transparent white transparent transparent;
  }

  .message-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 8px;
  }

  .message-sender {
    font-weight: 600;
    color: #333;
  }

  .message-time {
    color: #666;
    font-size: 0.8rem;
  }

  .message-subject {
    font-weight: 600;
    color: #667eea;
    margin-bottom: 8px;
  }

  .message-content {
    color: #555;
    line-height: 1.5;
  }

  .chat-message.deleted {
    opacity: 0.6;
  }

  .chat-message.deleted .message-bubble {
    background: #f8f9fa;
    border-color: #dee2e6;
  }

  .chat-message.deleted .message-sender,
  .chat-message.deleted .message-subject,
  .chat-message.deleted .message-content {
    color: #6c757d;
  }

  .deleted-indicator {
    color: #6c757d;
    font-size: 0.7rem;
    background: rgba(108, 117, 125, 0.1);
    padding: 2px 6px;
    border-radius: 4px;
  }









  .detail-sender {
    display: flex;
    align-items: center;
    gap: 15px;
    margin-bottom: 20px;
  }

  .detail-avatar {
    width: 36px;
    height: 36px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 1rem;
    color: white;
    overflow: hidden;
  }

  .detail-from {
    font-weight: 600;
    color: #333;
    margin-bottom: 5px;
  }

  .detail-time {
    color: #666;
    font-size: 0.9rem;
  }

  .detail-subject {
    color: #333;
    margin: 0 0 20px 0;
    font-size: 1.2rem;
    line-height: 1.4;
  }

  .detail-body {
    color: #555;
    line-height: 1.6;
    margin-bottom: 30px;
  }

  /* 删除确认对话框样式 */
  .delete-confirm-overlay {
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
    animation: fadeIn 0.2s ease-out;
  }

  @keyframes fadeIn {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }

  .delete-confirm-dialog {
    background: white;
    border-radius: 12px;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.2);
    max-width: 400px;
    width: 90%;
    overflow: hidden;
    animation: slideUp 0.3s cubic-bezier(0.25, 0.46, 0.45, 0.94);
  }

  @keyframes slideUp {
    from {
      transform: translateY(20px);
      opacity: 0;
    }
    to {
      transform: translateY(0);
      opacity: 1;
    }
  }

  .dialog-header {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 16px 24px 12px 24px;
    border-bottom: 1px solid #f1f3f4;
  }

  .dialog-header h3 {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
    color: #1f2937;
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .dialog-icon {
    width: 48px;
    height: 48px;
    border-radius: 50%;
    background: linear-gradient(135deg, #ff6b6b, #ee5a52);
    display: flex;
    align-items: center;
    justify-content: center;
    color: white;
    flex-shrink: 0;
  }

  .dialog-title {
    font-size: 18px;
    font-weight: 600;
    color: #212529;
    margin: 0;
  }

  .dialog-content {
    padding: 20px 24px;
    flex: 1;
    overflow-y: auto;
    min-height: 0; /* 确保flex子元素能正确收缩 */
    max-height: calc(100vh - 200px); /* 防止对话框过高 */
  }

  .dialog-message {
    font-size: 14px;
    color: #6c757d;
    line-height: 1.5;
    margin: 0 0 16px 0;
  }



  .dialog-actions {
    display: flex;
    gap: 12px;
    padding: 12px 24px 16px 24px;
    justify-content: flex-end;
    border-top: 1px solid #f1f3f4;
    flex-shrink: 0;
    background: white;
  }

  .dialog-btn {
    padding: 8px 20px;
    border: none;
    border-radius: 10px;
    font-size: 14px;
    font-weight: 500;
    cursor: pointer;
    transition: none;
    min-width: 80px;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    position: relative;
    overflow: hidden;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
  }

  .cancel-btn {
    background: linear-gradient(135deg, #6b7280 0%, #4b5563 100%);
    color: white;
  }

  .cancel-btn::before {
    content: '';
    position: absolute;
    top: 0;
    left: -100%;
    width: 100%;
    height: 100%;
    background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.3), transparent);
    transition: left 0.5s ease;
  }

  .cancel-btn:hover::before {
    left: 100%;
  }

  .confirm-btn {
    background: linear-gradient(135deg, var(--theme-primary) 0%, var(--theme-secondary) 100%);
    color: white;
  }

  .confirm-btn::before {
    content: '';
    position: absolute;
    top: 0;
    left: -100%;
    width: 100%;
    height: 100%;
    background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.3), transparent);
    transition: left 0.5s ease;
  }

  .confirm-btn:hover::before {
    left: 100%;
  }

  .confirm-btn:disabled {
    background: linear-gradient(135deg, #9ca3af 0%, #6b7280 100%);
    cursor: not-allowed;
  }

  .confirm-btn:disabled::before {
    display: none;
  }

  /* 简洁欢迎界面样式 */
  .simple-welcome {
    grid-column: 1 / -1; /* 跨越所有网格列 */
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    min-height: 60vh; /* 使用视口高度确保居中 */
    padding: 60px 20px;
  }

  .simple-welcome .welcome-icon {
    margin-bottom: 24px;
    color: var(--theme-primary);
    opacity: 0.7;
  }

  .simple-welcome .welcome-image {
    width: 250px;
    height: 250px;
    object-fit: contain;
  }



  .simple-welcome .welcome-subtitle {
    font-size: 18px;
    color: #6c757d;
    margin: -30px 0 16px 0;
    font-weight: 500;
  }

  .simple-welcome .welcome-hint {
    font-size: 14px;
    color: #868e96;
    margin: 0;
    line-height: 1.5;
    max-width: 400px;
  }

  .simple-welcome .welcome-features {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 12px;
    margin: 24px 0;
    width: 100%;
    max-width: 400px;
  }

  .simple-welcome .feature-item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 16px;
    background: rgba(255, 255, 255, 0.6);
    border-radius: 10px;
    border: 1px solid rgba(108, 122, 224, 0.1);
    transition: all 0.3s ease;
    font-size: 13px;
    font-weight: 500;
    color: #495057;
  }

  .simple-welcome .feature-item:hover {
    transform: translateY(-2px);
    box-shadow: 0 6px 20px rgba(108, 122, 224, 0.15);
    border-color: rgba(108, 122, 224, 0.2);
    background: rgba(255, 255, 255, 0.8);
  }

  .simple-welcome .feature-icon-img {
    width: 32px;
    height: 32px;
    flex-shrink: 0;
    object-fit: contain;
  }



  /* 响应式设计 */
  @media (max-width: 768px) {
    .simple-welcome .welcome-image {
      width: 150px;
      height: 150px;
    }



    .simple-welcome .welcome-subtitle {
      font-size: 16px;
      margin: -20px 0 12px 0;
    }

    .simple-welcome .welcome-features {
      grid-template-columns: 1fr;
      gap: 10px;
      margin: 20px 0;
    }

    .simple-welcome .feature-item {
      padding: 10px 14px;
      font-size: 12px;
    }

    .simple-welcome .welcome-hint {
      font-size: 13px;
    }
  }

  /* 回复对话框样式 */
  .reply-dialog-overlay {
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
  }

  .overlay-close-btn {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: transparent;
    border: none;
    cursor: default;
    z-index: -1; /* 放在对话框后面 */
  }

  .reply-dialog {
    background: linear-gradient(135deg, #ffffff 0%, #f8f9fa 100%);
    border-radius: 20px;
    width: 95%;
    max-width: 1000px;
    max-height: 85vh;
    overflow: hidden;
    box-shadow:
      0 25px 50px rgba(0, 0, 0, 0.15),
      0 10px 20px rgba(0, 0, 0, 0.1),
      0 0 0 1px rgba(255, 255, 255, 0.8);
    position: relative;
    z-index: 1;
    display: flex;
    flex-direction: column;
    backdrop-filter: blur(10px);
    border: 1px solid rgba(255, 255, 255, 0.2);
  }

  .reply-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px 20px;
    border-bottom: 1px solid rgba(0, 0, 0, 0.08);
    background: linear-gradient(135deg, #f8faff 0%, #f1f5ff 100%); /* 非常淡的蓝色背景 */
    color: #374151;
    position: relative;
  }

  /* 移除渐变背景效果 */

  .reply-header h3 {
    margin: 0;
    color: var(--theme-primary);
    font-size: 16px;
    font-weight: 600;
  }



  .reply-content {
    padding: 20px 0px; /* 移除左右内边距，让文本框能贴边 */
    flex: 1;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: linear-gradient(135deg, #fafbfc 0%, #f8f9fa 100%);
  }

  .reply-header-fields {
    display: flex;
    gap: 16px;
    margin-bottom: 16px;
    padding: 0 20px; /* 为顶部字段添加左右内边距 */
  }

  .reply-header-fields .reply-field {
    flex: 1;
    margin-bottom: 0;
  }

  .reply-main-content {
    display: flex;
    gap: 16px;
    flex: 1;
    min-height: 0;
    overflow: hidden;
    padding: 0 20px 0 0; /* 只给右侧内边距，左侧贴边 */
  }

  .reply-compose-area {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
    padding: 0 0 0 20px !important; /* 只给左侧一点内边距 */
    margin: 0 !important;
  }

  .reply-compose-area .reply-field {
    flex: 1;
    display: flex;
    flex-direction: column;
    margin: 0 !important;
    padding: 0 !important;
    width: 100%;
    box-sizing: border-box;
  }

  .reply-compose-area .reply-field label {
    margin: 0 0 8px 0 !important;
    padding: 0 !important;
    color: #374151;
    font-weight: 600;
    font-size: 14px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }



  /* 回复富文本编辑器样式 - 与转发对话框完全一致 */
  .reply-editor-toolbar {
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

  .reply-editor-toolbar .toolbar-group {
    display: flex;
    gap: 4px;
    position: relative;
  }

  .reply-editor-toolbar .editor-btn {
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

  .reply-editor-toolbar .editor-btn:hover {
    background: #f3f4f6;
    border-color: #9ca3af;
  }

  .reply-editor-toolbar .editor-btn:active {
    background: #e5e7eb;
  }

  /* 下拉按钮样式 */
  .reply-editor-toolbar .editor-btn.dropdown-btn {
    border: 1px solid #d1d5db;
    background: white;
  }

  .reply-editor-toolbar .editor-btn.dropdown-btn:hover {
    background: #f3f4f6;
    border-color: #9ca3af;
  }

  .reply-editor-toolbar .dropdown {
    position: relative;
  }

  .reply-editor-toolbar .dropdown-menu {
    position: absolute;
    top: 100%;
    left: 0;
    background: white;
    border: 1px solid #d1d5db;
    border-radius: 6px;
    box-shadow: 0 4px 6px rgba(0, 0, 0, 0.1);
    z-index: 1000;
    min-width: 120px;
  }

  .reply-editor-toolbar .dropdown-menu button {
    display: block;
    width: 100%;
    padding: 8px 12px;
    border: none;
    background: none;
    text-align: left;
    cursor: pointer;
    font-size: 0.85rem;
  }

  .reply-editor-toolbar .dropdown-menu button:hover {
    background: #f3f4f6;
  }

  .reply-editor-toolbar .color-picker {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 4px;
    padding: 8px;
  }

  .reply-editor-toolbar .color-btn {
    width: 24px;
    height: 24px;
    border: 1px solid #d1d5db;
    border-radius: 4px;
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .reply-editor-toolbar .color-btn:hover {
    transform: scale(1.1);
    border-color: #9ca3af;
  }

  /* 回复工具栏移动端响应式 */
  @media (max-width: 768px) {
    .reply-editor-toolbar {
      flex-wrap: wrap;
      gap: 4px;
      padding: 8px;
    }

    .reply-editor-toolbar .editor-btn {
      min-width: 28px;
      height: 28px;
      font-size: 0.8rem;
    }
  }

  .reply-rich-editor {
    min-height: 200px;
    max-height: 400px;
    padding: 16px;
    border: 2px solid #e5e7eb;
    border-radius: 12px;
    background: white;
    font-family: inherit;
    font-size: 0.9rem;
    line-height: 1.6;
    outline: none;
    overflow-y: auto;
    transition: all 0.3s ease;
  }

  .reply-rich-editor:focus {
    border-color: #667eea;
    box-shadow: 0 0 0 4px rgba(102, 126, 234, 0.1);
    transform: translateY(-1px);
  }

  .reply-rich-editor:hover {
    border-color: #d1d5db;
  }

  /* 回复编辑器占位符效果 */
  .reply-rich-editor:empty::before {
    content: attr(data-placeholder);
    color: #9ca3af;
    font-style: italic;
    pointer-events: none;
    position: absolute;
  }

  .original-message-area {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0; /* 防止flex子元素溢出 */
  }

  .reply-field {
    margin-bottom: 20px;
  }

  .reply-field label {
    display: block;
    margin-bottom: 8px;
    color: #374151;
    font-weight: 600;
    font-size: 14px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .reply-field input {
    width: 100%;
    padding: 14px 16px;
    border: none;
    border-radius: 12px;
    font-size: 14px;
    font-family: inherit;
    transition: all 0.3s ease;
    box-sizing: border-box;
    background: white;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.08);
  }

  .reply-field input:focus {
    outline: none;
    box-shadow:
      0 0 0 3px rgba(108, 122, 224, 0.15),
      0 6px 16px rgba(0, 0, 0, 0.12);
    transform: translateY(-2px);
  }

  .reply-field input[readonly] {
    background: linear-gradient(135deg, #f9fafb 0%, #f3f4f6 100%);
    color: #6b7280;
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.04);
  }



  .reply-actions {
    display: flex;
    justify-content: flex-end;
    gap: 12px; /* 减小按钮间距 */
    padding: 12px 20px; /* 大幅减小内边距，降低高度 */
    border-top: 1px solid rgba(0, 0, 0, 0.06);
    background: linear-gradient(135deg, #f8fafc 0%, #f1f5f9 100%);
    flex-shrink: 0;
    position: relative;
  }

  .reply-actions::before {
    content: '';
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 1px;
    background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.8), transparent);
  }

  .reply-btn {
    padding: 8px 20px; /* 减小按钮内边距 */
    border: none; /* 移除透明边框 */
    border-radius: 10px; /* 稍微减小圆角 */
    font-size: 13px; /* 稍微减小字体 */
    font-weight: 600;
    cursor: pointer;
    transition: all 0.3s ease;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    position: relative;
    overflow: hidden;
  }

  .reply-btn::before {
    content: '';
    position: absolute;
    top: 0;
    left: -100%;
    width: 100%;
    height: 100%;
    background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.3), transparent);
    transition: left 0.5s ease;
  }

  .reply-btn:hover::before {
    left: 100%;
  }

  .reply-btn.cancel {
    background: linear-gradient(135deg, #6b7280 0%, #4b5563 100%);
    color: white;
    box-shadow: 0 4px 12px rgba(107, 114, 128, 0.3);
  }



  .reply-btn.send {
    background: linear-gradient(135deg, var(--theme-primary) 0%, var(--theme-secondary) 100%);
    color: white;
    box-shadow: 0 4px 12px rgba(108, 122, 224, 0.3);
  }



  /* 原始邮件引用样式 */
  .original-message {
    border: none; /* 移除边框颜色 */
    border-radius: 16px;
    overflow: hidden;
    height: 100%;
    display: flex;
    flex-direction: column;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.05);
    background: white;
  }

  .original-message-header {
    background: linear-gradient(135deg, #f8fafc 0%, #f1f5f9 100%);
    padding: 14px 18px; /* 稍微增加内边距，让标题更突出 */
    border-bottom: 1px solid rgba(0, 0, 0, 0.06);
    position: relative;
  }

  /* 移除顶部彩色线条 */

  .original-message-title {
    font-weight: 700;
    color: #374151;
    font-size: 13px; /* 调大标题字体 */
    text-transform: uppercase;
    letter-spacing: 0.8px;
    margin: 0;
  }

  .original-message-content {
    padding: 12px;
    background: linear-gradient(135deg, #fefefe 0%, #fafbfc 100%);
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    /* 为缩放后的内容调整容器 */
    position: relative;
  }

  .original-message-meta {
    margin-bottom: 12px;
    font-size: 11px; /* 保持较小字体 */
    line-height: 1.5; /* 保持紧凑行高 */
    background: white;
    padding: 10px; /* 保持较小内边距 */
    border-radius: 8px; /* 保持较小圆角 */
    border: 1px solid #f0f0f0;
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.02);
    /* 移除缩放，保持原始大小 */
  }

  .original-message-meta div {
    margin-bottom: 4px;
    color: #4b5563;
    display: flex;
    align-items: center;
  }

  .original-message-meta strong {
    color: #1f2937;
    font-weight: 600;
    min-width: 50px;
    display: inline-block;
    margin-right: 6px;
  }

  .original-message-body {
    background: white;
    border: 1px solid #f0f0f0;
    border-radius: 8px;
    padding: 0;
    font-size: 12px;
    line-height: 1.4;
    color: #374151;
    flex: 1;
    overflow-y: auto;
    overflow-x: hidden; /* 隐藏水平滚动条 */
    word-wrap: break-word;
    box-shadow: 0 2px 4px rgba(0, 0, 0, 0.03);
    display: flex;
    flex-direction: column;
  }

  /* 自定义滚动条样式 */
  .original-message-body::-webkit-scrollbar {
    width: 4px; /* 更细的滚动条 */
  }

  .original-message-body::-webkit-scrollbar-track {
    background: transparent;
  }

  .original-message-body::-webkit-scrollbar-thumb {
    background: rgba(156, 163, 175, 0.3); /* 淡灰色 */
    border-radius: 2px;
    transition: background 0.2s ease;
  }

  .original-message-body::-webkit-scrollbar-thumb:hover {
    background: rgba(156, 163, 175, 0.5); /* 悬停时稍深 */
  }

  .original-message-body::-webkit-scrollbar-corner {
    background: transparent;
  }

  /* 复制邮件详情面板的样式 */
  .original-message-body .html-content {
    /* 完全保持邮件原始样式，不添加任何可能影响显示的样式 */
    padding: 12px; /* 增加内边距 */
    font-size: 11px; /* 直接缩小字体，而不是用transform */
    line-height: 1.3;
  }

  .original-message-body .text-content {
    white-space: pre-wrap;
    word-wrap: break-word;
    overflow-wrap: break-word;
    max-width: 100%;
    padding: 12px; /* 增加内边距 */
    font-size: 11px; /* 直接缩小字体，而不是用transform */
    line-height: 1.3;
  }

  .original-message-body .text-content pre {
    white-space: pre-wrap;
    word-wrap: break-word;
    font-family: inherit;
    margin: 0;
    line-height: 1.6;
    background: transparent; /* 保持邮件原始背景 */
    padding: 0; /* 移除额外的内边距 */
    border-radius: 0; /* 移除圆角 */
    /* 移除边框，保持邮件原始样式 */
  }

  .original-message-body .no-content-detail {
    text-align: center;
    color: #6c757d;
    padding: 40px 20px;
  }

  .original-message-body .no-content-icon {
    font-size: 48px;
    margin-bottom: 16px;
    opacity: 0.6;
  }

  .original-message-body .no-content-detail h4 {
    margin: 0 0 8px 0;
    font-size: 16px;
    font-weight: 600;
    color: #495057;
  }

  .original-message-body .no-content-detail p {
    margin: 0;
    font-size: 14px;
    color: #6c757d;
  }

  /* 分隔线样式 */
  .separator-line {
    width: 100%;
    height: 1px;
    background-color: #d1d5db;
    margin: 16px 0;
    border: none;
  }

  /* 保存草稿确认对话框样式 */
  .save-draft-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 2000; /* 比回复对话框更高 */
    backdrop-filter: blur(4px);
    animation: fadeIn 0.2s ease-out;
  }

  .save-draft-dialog {
    background: linear-gradient(135deg, #ffffff 0%, #f8f9fa 100%);
    border-radius: 16px;
    padding: 0;
    width: 90%;
    max-width: 400px;
    box-shadow:
      0 25px 50px rgba(0, 0, 0, 0.15),
      0 10px 20px rgba(0, 0, 0, 0.1),
      0 0 0 1px rgba(255, 255, 255, 0.8);
    overflow: hidden;
    animation: scaleIn 0.2s ease-out;
  }

  .save-draft-header {
    padding: 20px 24px 16px;
    border-bottom: 1px solid rgba(0, 0, 0, 0.1);
  }

  .save-draft-header h3 {
    margin: 0;
    font-size: 18px;
    font-weight: 600;
    color: #1f2937;
  }

  .save-draft-content {
    padding: 20px 24px;
  }

  .save-draft-message {
    margin: 0;
    font-size: 14px;
    color: #6b7280;
    line-height: 1.5;
  }

  .save-draft-actions {
    padding: 16px 24px 20px;
    display: flex;
    gap: 12px;
    justify-content: flex-end;
  }

  .save-draft-btn {
    padding: 10px 20px;
    border-radius: 8px;
    border: none;
    cursor: pointer;
    font-weight: 500;
    font-size: 14px;
    transition: all 0.2s ease;
  }



  .save-draft-btn.discard-btn {
    background: linear-gradient(135deg, #ff6b6b 0%, #ee5a52 100%);
    color: white;
  }

  .save-draft-btn.discard-btn:hover {
    background: linear-gradient(135deg, #ff5252 0%, #e53935 100%);
    transform: translateY(-1px);
    box-shadow: 0 4px 12px rgba(255, 107, 107, 0.3);
  }

  .save-draft-btn.confirm-btn {
    background: linear-gradient(135deg, var(--theme-primary) 0%, var(--theme-secondary) 100%);
    color: white;
  }

  .save-draft-btn.confirm-btn:hover {
    background: linear-gradient(135deg, var(--theme-hover) 0%, var(--theme-hover-secondary) 100%);
    transform: translateY(-1px);
    box-shadow: 0 4px 12px rgba(59, 130, 246, 0.3);
  }

  @keyframes scaleIn {
    from {
      opacity: 0;
      transform: scale(0.9);
    }
    to {
      opacity: 1;
      transform: scale(1);
    }
  }

  /* 设置页面样式 */
  .settings-page {
    flex: 1;
    display: flex;
    flex-direction: column;
    background: linear-gradient(135deg,
      #f8f9fa 0%,
      #ffffff 25%,
      #f1f3f4 50%,
      #ffffff 75%,
      #f8f9fa 100%);
    height: 100%;
    overflow: hidden;
    width: 100%;
  }

  .settings-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px 20px;
    border-bottom: 1px solid #e9ecef;
    background: #f8f9fa;
    height: 48px;
    box-sizing: border-box;
  }

  .settings-header h2 {
    margin: 0;
    font-size: 18px;
    font-weight: 600;
    color: #495057;
  }

  .close-settings-btn {
    background: #e9ecef;
    border: none;
    border-radius: 6px;
    padding: 6px;
    cursor: pointer;
    color: #6c757d;
    transition: all 0.2s ease;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
  }

  .close-settings-btn:hover {
    background: linear-gradient(135deg, var(--theme-primary), var(--theme-secondary));
    color: white;
  }

  .settings-content {
    flex: 1;
    display: flex;
    overflow: hidden;
  }

  .settings-sidebar {
    width: 240px;
    background: #f8f9fa;
    border-right: 1px solid #e9ecef;
    padding: 16px 0;
  }

  .settings-nav {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 0 16px;
  }

  .settings-nav-item {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 16px;
    background: transparent;
    border: none;
    border-radius: 8px;
    cursor: pointer;
    color: #6c757d;
    font-size: 14px;
    font-weight: 500;
    transition: all 0.2s ease;
    text-align: left;
    width: 100%;
  }

  .settings-nav-item:hover {
    background: linear-gradient(135deg, rgba(108, 122, 224, 0.1), rgba(123, 104, 238, 0.1));
    color: var(--theme-primary);
  }

  .settings-nav-item.active {
    background: linear-gradient(135deg, var(--theme-primary), var(--theme-secondary));
    color: white;
  }

  .settings-nav-item svg {
    flex-shrink: 0;
  }

  .settings-main {
    flex: 1;
    padding: 24px;
    overflow-y: auto;
    position: relative;
    /* Firefox 滚动条样式 */
    scrollbar-width: thin;
    scrollbar-color: rgba(0, 0, 0, 0.2) transparent;
  }

  /* 设置页面自定义滚动条样式 - Webkit浏览器 */
  .settings-main::-webkit-scrollbar {
    width: 6px;
  }

  .settings-main::-webkit-scrollbar-track {
    background: transparent;
  }

  .settings-main::-webkit-scrollbar-thumb {
    background: rgba(0, 0, 0, 0.2);
    border-radius: 3px;
  }

  .settings-main::-webkit-scrollbar-thumb:hover {
    background: rgba(0, 0, 0, 0.3);
  }

  .settings-main::-webkit-scrollbar-corner {
    background: transparent;
  }

  .settings-section {
    max-width: 600px;
  }

  .settings-section h3 {
    margin: 0 0 8px 0;
    font-size: 20px;
    font-weight: 600;
    color: #1f2937;
  }

  .settings-description {
    margin: 0 0 24px 0;
    color: #6b7280;
    font-size: 14px;
    line-height: 1.5;
  }

  .settings-placeholder {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 48px 24px;
    background: #f8f9fa;
    border-radius: 12px;
    border: 2px dashed #dee2e6;
    text-align: center;
  }

  .placeholder-icon {
    font-size: 48px;
    margin-bottom: 16px;
    opacity: 0.6;
  }

  .settings-placeholder p {
    margin: 0;
    color: #6b7280;
    font-size: 16px;
  }

  /* 设置项样式 */
  .setting-item {
    margin-bottom: 32px;
    padding: 20px;
    background: rgba(255, 255, 255, 0.85);
    backdrop-filter: blur(10px);
    -webkit-backdrop-filter: blur(10px);
    border: 1px solid rgba(255, 255, 255, 0.3);
    border-radius: 16px;
    box-shadow:
      0 8px 32px rgba(0, 0, 0, 0.1),
      0 2px 8px rgba(0, 0, 0, 0.05),
      inset 0 1px 0 rgba(255, 255, 255, 0.2);
    transition: all 0.3s ease;
  }

  .setting-item:hover {
    background: rgba(255, 255, 255, 0.9);
    box-shadow:
      0 12px 40px rgba(0, 0, 0, 0.15),
      0 4px 12px rgba(0, 0, 0, 0.1),
      inset 0 1px 0 rgba(255, 255, 255, 0.3);
  }

  .setting-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 8px;
  }

  .setting-header h4 {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
    color: #1f2937;
  }

  .setting-description {
    margin: 0;
    color: #6b7280;
    font-size: 14px;
    line-height: 1.5;
  }

  /* 开关切换样式 */
  .toggle-switch {
    position: relative;
    display: inline-block;
    width: 48px;
    height: 24px;
  }

  :global(.toggle-switch input) {
    opacity: 0;
    width: 0;
    height: 0;
  }

  .toggle-slider {
    position: absolute;
    cursor: pointer;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background-color: #ccc;
    transition: 0.3s;
    border-radius: 24px;
  }

  .toggle-slider:before {
    position: absolute;
    content: "";
    height: 18px;
    width: 18px;
    left: 3px;
    bottom: 3px;
    background-color: white;
    transition: 0.3s;
    border-radius: 50%;
  }

  :global(input:checked + .toggle-slider) {
    background: linear-gradient(135deg, var(--theme-primary), var(--theme-secondary));
  }

  :global(input:checked + .toggle-slider:before) {
    transform: translateX(24px);
  }

  /* 铃声选择样式 */
  .sound-options {
    display: flex;
    flex-direction: column;
    gap: 12px;
    margin-top: 16px;
  }

  .sound-option {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 16px;
    background: rgba(248, 249, 250, 0.7);
    backdrop-filter: blur(8px);
    -webkit-backdrop-filter: blur(8px);
    border: 1px solid rgba(233, 236, 239, 0.5);
    border-radius: 12px;
    transition: all 0.3s ease;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.05);
  }

  .sound-option:hover {
    background: rgba(233, 236, 239, 0.8);
    transform: translateY(-1px);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
  }

  .sound-radio {
    display: flex;
    align-items: center;
    gap: 12px;
    cursor: pointer;
    flex: 1;
  }

  :global(.sound-radio input[type="radio"]) {
    display: none;
  }

  .radio-mark {
    width: 16px;
    height: 16px;
    border: 2px solid #dee2e6;
    border-radius: 50%;
    position: relative;
    transition: all 0.2s ease;
  }

  :global(.sound-radio input[type="radio"]:checked + .radio-mark) {
    border-color: var(--theme-primary);
    background: linear-gradient(135deg, var(--theme-primary), var(--theme-secondary));
  }

  :global(.sound-radio input[type="radio"]:checked + .radio-mark:after) {
    content: '';
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: 6px;
    height: 6px;
    background: white;
    border-radius: 50%;
  }

  .sound-name {
    font-size: 14px;
    font-weight: 500;
    color: #495057;
  }

  .test-sound-btn {
    background: linear-gradient(135deg, var(--theme-primary), var(--theme-secondary));
    border: none;
    border-radius: 6px;
    padding: 8px;
    cursor: pointer;
    color: white;
    transition: all 0.2s ease;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .test-sound-btn:hover {
    background: linear-gradient(135deg, var(--theme-hover), var(--theme-hover-secondary));
    transform: translateY(-1px);
  }

  :global(.test-sound-btn svg) {
    width: 16px;
    height: 16px;
  }



  /* 朦胧气泡背景效果 */
  .bubble-background {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    pointer-events: none;
    overflow: hidden;
    z-index: 0;
  }

  .bubble {
    position: absolute;
    border-radius: 50%;
    background: linear-gradient(135deg,
      rgba(59, 130, 246, 0.1) 0%,
      rgba(147, 197, 253, 0.15) 50%,
      rgba(219, 234, 254, 0.1) 100%);
    backdrop-filter: blur(20px);
    -webkit-backdrop-filter: blur(20px);
    animation: float 6s ease-in-out infinite;
    box-shadow:
      0 8px 32px rgba(59, 130, 246, 0.1),
      inset 0 1px 0 rgba(255, 255, 255, 0.2);
  }

  .bubble-1 {
    width: 120px;
    height: 120px;
    top: 10%;
    left: 15%;
    animation-delay: 0s;
    animation-duration: 8s;
    background: linear-gradient(135deg,
      rgba(59, 130, 246, 0.12) 0%,
      rgba(147, 197, 253, 0.18) 50%,
      rgba(219, 234, 254, 0.12) 100%);
  }

  .bubble-2 {
    width: 80px;
    height: 80px;
    top: 60%;
    right: 20%;
    animation-delay: -2s;
    animation-duration: 10s;
    background: linear-gradient(135deg,
      rgba(168, 85, 247, 0.1) 0%,
      rgba(196, 181, 253, 0.15) 50%,
      rgba(233, 213, 255, 0.1) 100%);
  }

  .bubble-3 {
    width: 60px;
    height: 60px;
    top: 30%;
    right: 10%;
    animation-delay: -4s;
    animation-duration: 7s;
    background: linear-gradient(135deg,
      rgba(34, 197, 94, 0.1) 0%,
      rgba(134, 239, 172, 0.15) 50%,
      rgba(187, 247, 208, 0.1) 100%);
  }

  .bubble-4 {
    width: 100px;
    height: 100px;
    bottom: 20%;
    left: 10%;
    animation-delay: -1s;
    animation-duration: 9s;
    background: linear-gradient(135deg,
      rgba(239, 68, 68, 0.1) 0%,
      rgba(252, 165, 165, 0.15) 50%,
      rgba(254, 202, 202, 0.1) 100%);
  }

  .bubble-5 {
    width: 40px;
    height: 40px;
    top: 80%;
    left: 60%;
    animation-delay: -3s;
    animation-duration: 6s;
    background: linear-gradient(135deg,
      rgba(245, 158, 11, 0.1) 0%,
      rgba(251, 191, 36, 0.15) 50%,
      rgba(254, 240, 138, 0.1) 100%);
  }

  .bubble-6 {
    width: 90px;
    height: 90px;
    top: 5%;
    right: 40%;
    animation-delay: -5s;
    animation-duration: 11s;
    background: linear-gradient(135deg,
      rgba(236, 72, 153, 0.1) 0%,
      rgba(251, 113, 133, 0.15) 50%,
      rgba(252, 165, 165, 0.1) 100%);
  }

  @keyframes float {
    0%, 100% {
      transform: translateY(0px) translateX(0px) scale(1);
      opacity: 0.7;
    }
    25% {
      transform: translateY(-20px) translateX(10px) scale(1.05);
      opacity: 0.9;
    }
    50% {
      transform: translateY(-10px) translateX(-15px) scale(0.95);
      opacity: 0.8;
    }
    75% {
      transform: translateY(-30px) translateX(5px) scale(1.02);
      opacity: 0.85;
    }
  }

  /* 确保设置内容在气泡之上 */
  .settings-section {
    position: relative;
    z-index: 1;
  }

  /* 动画预览样式 */
  .animation-style-section {
    margin-top: 16px;
  }

  .animation-preview {
    width: 100%;
    height: 120px;
    background: #f8f9fa;
    border-radius: 8px;
    overflow: hidden;
    position: relative;
    border: 1px solid #e9ecef;
  }

  .preview-container {
    width: 100%;
    height: 100%;
    position: relative;
    background: linear-gradient(135deg, #f1f3f4 0%, #ffffff 100%);
  }

  .preview-email-card {
    position: absolute;
    top: 10px;
    left: 10px;
    right: 50%;
    height: 40px;
    background: white;
    border-radius: 6px;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
    display: flex;
    align-items: center;
    padding: 8px 12px;
  }

  .preview-card-content {
    flex: 1;
  }

  .preview-subject {
    font-size: 11px;
    font-weight: 600;
    color: #333;
    margin-bottom: 2px;
  }

  .preview-sender {
    font-size: 9px;
    color: #666;
  }

  .preview-detail {
    position: absolute;
    background: white;
    border-radius: 8px;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.15);
    padding: 12px;
  }

  .slideup-animation {
    bottom: 10px;
    left: 10px;
    right: 10px;
    height: 60px;
    animation: previewSlideUp 2s infinite;
  }

  .slideright-animation {
    top: 10px;
    bottom: 10px;
    left: 55%;
    right: 10px;
    animation: previewSlideRight 2s infinite;
  }

  .slideleft-animation {
    top: 10px;
    bottom: 10px;
    left: 10px;
    right: 55%;
    animation: previewSlideLeft 2s infinite;
  }

  .preview-detail-header {
    font-size: 10px;
    font-weight: 600;
    color: #333;
    margin-bottom: 4px;
  }

  .preview-detail-content {
    font-size: 8px;
    color: #666;
    line-height: 1.3;
  }

  @keyframes previewSlideUp {
    0%, 20% {
      transform: translateY(100%);
      opacity: 0;
    }
    30%, 70% {
      transform: translateY(0);
      opacity: 1;
    }
    80%, 100% {
      transform: translateY(100%);
      opacity: 0;
    }
  }

  @keyframes previewSlideRight {
    0%, 20% {
      transform: translateX(-100%);
      opacity: 0;
    }
    30%, 70% {
      transform: translateX(0);
      opacity: 1;
    }
    80%, 100% {
      transform: translateX(-100%);
      opacity: 0;
    }
  }

  @keyframes previewSlideLeft {
    0%, 20% {
      transform: translateX(100%);
      opacity: 0;
    }
    30%, 70% {
      transform: translateX(0);
      opacity: 1;
    }
    80%, 100% {
      transform: translateX(100%);
      opacity: 0;
    }
  }

  .animation-info {
    margin-top: 16px;
  }

  /* 用户信息卡片样式 */
  .user-info-card {
    background: rgba(255, 255, 255, 0.9);
    border-radius: 16px;
    padding: 24px;
    margin-bottom: 24px;
    display: flex;
    align-items: center;
    gap: 20px;
    backdrop-filter: blur(10px);
    border: 1px solid rgba(255, 255, 255, 0.2);
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.1);
  }

  .user-avatar {
    flex-shrink: 0;
  }

  .avatar-circle {
    width: 64px;
    height: 64px;
    border-radius: 50%;
    background: linear-gradient(135deg, var(--theme-primary), var(--theme-secondary));
    display: flex;
    align-items: center;
    justify-content: center;
    color: white;
    font-size: 24px;
    font-weight: 600;
    overflow: hidden;
    position: relative;
  }

  .avatar-circle .avatar-image {
    width: 100%;
    height: 100%;
    object-fit: cover;
    border-radius: 50%;
  }

  .user-details {
    flex: 1;
  }

  .user-name {
    margin: 0 0 8px 0;
    font-size: 20px;
    font-weight: 600;
    color: #1f2937;
  }

  .user-email {
    margin: 0 0 12px 0;
    font-size: 14px;
    color: #6b7280;
  }

  .auth-badge {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    border-radius: 20px;
    font-size: 12px;
    font-weight: 500;
  }

  .auth-badge.oauth2 {
    background: linear-gradient(135deg, #10b981, #059669);
    color: white;
  }

  .auth-badge.google {
    background: linear-gradient(135deg, #4285f4, #34a853);
    color: white;
  }

  .auth-badge.qq {
    background: linear-gradient(135deg, #12b7f5, #0099e5);
    color: white;
  }

  .auth-badge.password {
    background: linear-gradient(135deg, #6366f1, #4f46e5);
    color: white;
  }

  /* 账号操作按钮 */
  .account-actions {
    display: flex;
    gap: 12px;
    flex-wrap: wrap;
  }

  .settings-btn {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 10px 16px;
    border-radius: 8px;
    border: none;
    cursor: pointer;
    font-weight: 500;
    font-size: 14px;
    transition: all 0.2s ease;
    text-decoration: none;
  }

  .settings-btn.small {
    padding: 6px 12px;
    font-size: 12px;
  }

  .settings-btn.primary {
    background: linear-gradient(135deg, var(--theme-primary), var(--theme-secondary));
    color: white;
  }

  .settings-btn.primary:hover {
    transform: translateY(-1px);
    box-shadow: 0 4px 12px rgba(108, 122, 224, 0.3);
  }

  .settings-btn.secondary {
    background: linear-gradient(135deg, #6b7280 0%, #4b5563 100%);
    color: white;
    border: none;
    border-radius: 10px;
    font-size: 13px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    position: relative;
    overflow: hidden;
    box-shadow: 0 4px 12px rgba(107, 114, 128, 0.3);
    transition: all 0.3s ease;
  }

  .settings-btn.secondary::before {
    content: '';
    position: absolute;
    top: 0;
    left: -100%;
    width: 100%;
    height: 100%;
    background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.3), transparent);
    transition: left 0.5s ease;
  }

  .settings-btn.secondary:hover {
    background: linear-gradient(135deg, #4b5563, #374151);
    /* 移除悬浮效果 */
  }

  .settings-btn.secondary:hover::before {
    left: 100%;
  }

  .settings-btn.danger {
    background: linear-gradient(135deg, #ef4444, #dc2626);
    color: white;
    border-radius: 10px;
    font-size: 13px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    position: relative;
    overflow: hidden;
    box-shadow: 0 4px 12px rgba(239, 68, 68, 0.3);
    transition: all 0.3s ease;
  }

  .settings-btn.danger::before {
    content: '';
    position: absolute;
    top: 0;
    left: -100%;
    width: 100%;
    height: 100%;
    background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.3), transparent);
    transition: left 0.5s ease;
  }

  .settings-btn.danger:hover {
    background: linear-gradient(135deg, #dc2626, #b91c1c);
    /* 移除悬浮效果 */
  }

  .settings-btn.danger:hover::before {
    left: 100%;
  }

  /* 邮箱账号列表样式 */
  .email-accounts-list {
    display: flex;
    flex-direction: column;
    gap: 16px;
    margin-bottom: 24px;
  }

  .email-account-card {
    background: rgba(255, 255, 255, 0.9);
    border-radius: 12px;
    padding: 20px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    backdrop-filter: blur(10px);
    border: 1px solid rgba(255, 255, 255, 0.2);
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.1);
  }





  .settings-account-info {
    display: flex;
    align-items: center;
    gap: 16px;
    flex: 1;
  }

  .account-avatar {
    flex-shrink: 0;
  }

  .settings-provider-icon {
    width: 48px;
    height: 48px;
    border-radius: 12px;
    background: rgba(255, 255, 255, 0.9);
    display: flex;
    align-items: center;
    justify-content: center;
    border: 1px solid rgba(0, 0, 0, 0.1);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
  }

  .settings-provider-icon .provider-icon-img {
    width: 32px;
    height: 32px;
    object-fit: contain;
  }

  .account-details {
    flex: 1;
  }

  .settings-account-name {
    margin: 0 0 4px 0;
    font-size: 16px;
    font-weight: 600;
    color: #1f2937;
  }

  .settings-account-email {
    margin: 0 0 8px 0;
    font-size: 14px;
    color: #6b7280;
  }

  .account-status {
    display: flex;
    align-items: center;
  }

  /* 数据库路径设置样式 */
  .database-path-section {
    margin-top: 16px;
  }

  .current-path {
    margin-bottom: 16px;
  }

  .path-label {
    display: block;
    font-size: 14px;
    font-weight: 500;
    color: #374151;
    margin-bottom: 8px;
  }

  .path-display {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px;
    background: rgba(255, 255, 255, 0.8);
    border: 1px solid rgba(0, 0, 0, 0.1);
    border-radius: 8px;
    backdrop-filter: blur(10px);
  }

  .path-text {
    flex: 1;
    font-family: 'Consolas', 'Monaco', monospace;
    font-size: 13px;
    color: #1f2937;
    word-break: break-all;
  }

  .copy-path-btn {
    padding: 4px;
    background: transparent;
    border: none;
    border-radius: 4px;
    color: #6b7280;
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .copy-path-btn:hover {
    background: rgba(0, 0, 0, 0.05);
    color: #374151;
  }

  .database-actions {
    display: flex;
    gap: 12px;
    margin-bottom: 16px;
  }

  .database-info {
    padding: 12px;
    background: rgba(59, 130, 246, 0.1);
    border: 1px solid rgba(59, 130, 246, 0.2);
    border-radius: 8px;
  }

  .info-item {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 8px;
    font-size: 13px;
    color: #1e40af;
  }

  .info-item:last-child {
    margin-bottom: 0;
  }

  .info-item svg {
    flex-shrink: 0;
    color: #1e40af;
  }

  /* 数据库路径对话框样式 */
  .database-path-overlay {
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

  .database-path-dialog {
    background: white;
    border-radius: 16px;
    box-shadow: 0 20px 40px rgba(0, 0, 0, 0.15);
    width: 90%;
    max-width: 500px;
    max-height: 85vh;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    animation: dialogSlideIn 0.3s ease-out;
  }

  /* 数据库路径对话框滚动条样式 */
  .database-path-dialog::-webkit-scrollbar {
    width: 8px;
  }

  .database-path-dialog::-webkit-scrollbar-track {
    background: #f1f5f9;
    border-radius: 4px;
  }

  .database-path-dialog::-webkit-scrollbar-thumb {
    background: linear-gradient(135deg, #cbd5e1 0%, #94a3b8 100%);
    border-radius: 4px;
  }

  .database-path-dialog::-webkit-scrollbar-thumb:hover {
    background: linear-gradient(135deg, #94a3b8 0%, #64748b 100%);
  }

  .database-path-dialog::-webkit-scrollbar-button {
    display: none;
  }

  /* 数据库路径对话框内容区域样式 */
  .database-path-dialog .dialog-content {
    flex: 1;
    overflow-y: auto;
    min-height: 0;
    padding: 20px 24px;
  }

  /* 数据库路径对话框内容区域滚动条样式 */
  .database-path-dialog .dialog-content::-webkit-scrollbar {
    width: 8px;
  }

  .database-path-dialog .dialog-content::-webkit-scrollbar-track {
    background: #f1f5f9;
    border-radius: 4px;
  }

  .database-path-dialog .dialog-content::-webkit-scrollbar-thumb {
    background: linear-gradient(135deg, #cbd5e1 0%, #94a3b8 100%);
    border-radius: 4px;
  }

  .database-path-dialog .dialog-content::-webkit-scrollbar-thumb:hover {
    background: linear-gradient(135deg, #94a3b8 0%, #64748b 100%);
  }

  .database-path-dialog .dialog-content::-webkit-scrollbar-button {
    display: none;
  }

  .path-input-section {
    margin: 20px 0;
  }

  .path-input-section label {
    display: block;
    font-size: 14px;
    font-weight: 500;
    color: #374151;
    margin-bottom: 8px;
  }

  .path-input-group {
    display: flex;
    gap: 8px;
  }

  .path-input-group input {
    flex: 1;
    padding: 12px;
    border: 1px solid #d1d5db;
    border-radius: 8px;
    font-size: 14px;
    background: #f9fafb;
    color: #374151;
  }

  .browse-btn {
    padding: 8px 20px;
    border: none;
    border-radius: 10px;
    background: linear-gradient(135deg, var(--theme-primary) 0%, var(--theme-secondary) 100%);
    color: white;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    display: flex;
    align-items: center;
    gap: 6px;
    box-shadow: 0 4px 12px rgba(108, 122, 224, 0.3);
  }



  .migration-options {
    margin-bottom: 24px;
  }

  .option-item {
    margin-bottom: 16px;
  }

  .option-item:last-child {
    margin-bottom: 0;
  }

  .option-item input[type="radio"] {
    margin-right: 12px;
  }

  .option-item label {
    display: flex;
    align-items: flex-start;
    cursor: pointer;
    padding: 16px;
    border: 2px solid #e5e7eb;
    border-radius: 8px;
    transition: all 0.2s ease;
  }

  .option-item input[type="radio"]:checked + label {
    border-color: var(--theme-primary);
    background: linear-gradient(135deg, rgba(108, 122, 224, 0.1) 0%, rgba(255, 107, 107, 0.1) 100%);
  }

  .option-content {
    flex: 1;
  }

  .option-title {
    display: flex;
    align-items: center;
    gap: 8px;
    font-weight: 500;
    color: #374151;
    margin-bottom: 4px;
  }

  .option-title svg {
    flex-shrink: 0;
    color: #6b7280;
  }

  .option-description {
    font-size: 14px;
    color: #6b7280;
    line-height: 1.4;
  }

  .info-section {
    background: #f0f9ff;
    border: 1px solid #0ea5e9;
    border-radius: 8px;
    padding: 16px;
    margin-bottom: 24px;
  }

  .info-item {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 14px;
    color: #0c4a6e;
    margin-bottom: 8px;
  }

  .info-item:last-child {
    margin-bottom: 0;
  }

  .info-item svg {
    flex-shrink: 0;
    color: #0ea5e9;
  }

  .dialog-description {
    font-size: 14px;
    color: #6b7280;
    line-height: 1.5;
    margin: 0;
  }

  .status-badge {
    padding: 4px 8px;
    border-radius: 12px;
    font-size: 11px;
    font-weight: 500;
  }

  .status-badge.oauth2 {
    background: rgba(16, 185, 129, 0.1);
    color: #059669;
  }

  .status-badge.password {
    background: rgba(99, 102, 241, 0.1);
    color: #4f46e5;
  }

  .add-account-section {
    text-align: center;
    padding: 20px 0;
  }

  /* 主题选择样式 */
  .theme-options {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
    gap: 16px;
    margin-top: 16px;
  }

  .theme-option {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    padding: 16px;
    background: rgba(255, 255, 255, 0.9);
    border: 2px solid transparent;
    border-radius: 12px;
    cursor: pointer;
    transition: all 0.2s ease;
    backdrop-filter: blur(10px);
  }

  .theme-option:hover {
    transform: translateY(-2px);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.1);
  }

  .theme-option.active {
    border-color: var(--theme-primary);
    background: rgba(108, 122, 224, 0.1);
  }

  .theme-preview {
    display: flex;
    gap: 4px;
  }

  .theme-color {
    width: 24px;
    height: 24px;
    border-radius: 50%;
    border: 2px solid rgba(255, 255, 255, 0.8);
  }

  .theme-name {
    font-size: 12px;
    font-weight: 500;
    color: #374151;
    text-align: center;
  }

  /* 自定义主题选项样式 */
  .custom-theme-option {
    position: relative;
  }

  .custom-theme-actions {
    position: absolute;
    top: 8px;
    right: 8px;
    opacity: 0;
    transition: opacity 0.2s ease;
  }

  .custom-theme-option:hover .custom-theme-actions {
    opacity: 1;
  }

  .edit-custom-theme-btn {
    width: 24px;
    height: 24px;
    border: none;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.9);
    color: #6b7280;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.2s ease;
    box-shadow: 0 2px 4px rgba(0, 0, 0, 0.1);
  }

  .edit-custom-theme-btn:hover {
    background: white;
    color: var(--theme-primary);
    transform: scale(1.1);
  }

  /* 自定义主题对话框样式 */
  .dialog-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.3);
    backdrop-filter: blur(2px);
    z-index: 1000;
  }

  .custom-theme-dialog {
    background: white;
    border-radius: 16px 0 0 16px;
    width: 500px;
    height: 100vh;
    position: fixed;
    right: 0;
    top: 0;
    overflow: hidden;
    box-shadow: -10px 0 30px rgba(0, 0, 0, 0.3);
    display: flex;
    flex-direction: column;
    transform: translateX(100%);
    transition: transform 0.3s ease-out;
  }

  .custom-theme-dialog.show {
    transform: translateX(0);
  }

  .dialog-header {
    display: flex;
    justify-content: between;
    align-items: center;
    padding: 20px 24px;
    border-bottom: 1px solid #e5e7eb;
    background: #f9fafb;
  }

  .dialog-header h3 {
    margin: 0;
    font-size: 18px;
    font-weight: 600;
    color: #111827;
    flex: 1;
  }



  /* 移除重复的 .dialog-content 定义，避免样式冲突 */

  /* 自定义主题对话框滚动条样式 */
  .custom-theme-dialog .dialog-content::-webkit-scrollbar {
    width: 8px;
  }

  .custom-theme-dialog .dialog-content::-webkit-scrollbar-track {
    background: #f1f5f9;
    border-radius: 4px;
  }

  .custom-theme-dialog .dialog-content::-webkit-scrollbar-thumb {
    background: linear-gradient(135deg, #cbd5e1 0%, #94a3b8 100%);
    border-radius: 4px;
    transition: background 0.3s ease;
  }

  .custom-theme-dialog .dialog-content::-webkit-scrollbar-thumb:hover {
    background: linear-gradient(135deg, #94a3b8 0%, #64748b 100%);
  }

  /* 移除滚动条箭头按钮 */
  .custom-theme-dialog .dialog-content::-webkit-scrollbar-button {
    display: none;
  }

  /* Firefox 滚动条样式 */
  .custom-theme-dialog .dialog-content {
    scrollbar-width: thin;
    scrollbar-color: #cbd5e1 #f1f5f9;
    /* 重写对话框内容区域的高度限制 */
    max-height: none;
    flex: 1;
    min-height: 0;
  }

  /* 回复对话框自定义滚动条样式 */
  .reply-rich-editor::-webkit-scrollbar,
  .original-message-content::-webkit-scrollbar {
    width: 8px;
  }

  .reply-rich-editor::-webkit-scrollbar-track,
  .original-message-content::-webkit-scrollbar-track {
    background: #f1f5f9;
    border-radius: 4px;
  }

  .reply-rich-editor::-webkit-scrollbar-thumb,
  .original-message-content::-webkit-scrollbar-thumb {
    background: linear-gradient(135deg, #cbd5e1 0%, #94a3b8 100%);
    border-radius: 4px;
    transition: background 0.3s ease;
  }

  .reply-rich-editor::-webkit-scrollbar-thumb:hover,
  .original-message-content::-webkit-scrollbar-thumb:hover {
    background: linear-gradient(135deg, #94a3b8 0%, #64748b 100%);
  }

  /* 移除回复对话框滚动条箭头按钮 */
  .reply-rich-editor::-webkit-scrollbar-button,
  .original-message-content::-webkit-scrollbar-button {
    display: none;
  }

  /* Firefox 回复对话框滚动条样式 */
  .reply-rich-editor,
  .original-message-content {
    scrollbar-width: thin;
    scrollbar-color: #cbd5e1 #f1f5f9;
  }

  .theme-preview-section {
    margin-bottom: 24px;
  }

  .theme-preview-section h4 {
    margin: 0 0 12px 0;
    font-size: 14px;
    font-weight: 600;
    color: #111827;
  }

  .theme-preview-large {
    border-radius: 8px;
    overflow: hidden;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
  }

  .preview-card {
    padding: 16px;
    color: white;
    min-height: 80px;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .preview-content {
    text-align: center;
  }



  .preview-button {
    display: inline-block;
    padding: 6px 12px;
    margin: 3px 6px;
    border-radius: 6px;
    border: 2px solid transparent;
    color: white;
    font-weight: 500;
    font-size: 12px;
    transition: all 0.2s ease;
  }

  .preview-button:hover {
    transform: translateY(-1px);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.2);
  }

  .color-settings {
    display: grid;
    gap: 20px;
    padding: 8px;
  }

  .color-warning {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    padding: 16px;
    background: linear-gradient(135deg, #fef3cd 0%, #fde68a 100%);
    border: 1px solid #f59e0b;
    border-radius: 12px;
    margin-bottom: 8px;
  }

  .warning-icon {
    font-size: 18px;
    flex-shrink: 0;
    margin-top: 2px;
  }

  .warning-text {
    font-size: 14px;
    color: #92400e;
    line-height: 1.5;
  }

  .warning-text strong {
    color: #78350f;
    font-weight: 600;
  }

  .color-group {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 16px;
    background: linear-gradient(135deg, #ffffff 0%, #f8fafc 100%);
    border-radius: 16px;
    border: 1px solid rgba(0, 0, 0, 0.06);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.04);
  }

  .color-group label {
    font-size: 15px;
    font-weight: 600;
    color: #1f2937;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    margin-bottom: 4px;
  }

  .color-input-group {
    display: flex;
    gap: 16px;
    align-items: center;
  }

  .color-picker {
    width: 48px;
    height: 48px;
    border: none;
    border-radius: 50%;
    cursor: pointer;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.15);
    transition: all 0.3s ease;
    position: relative;
    overflow: hidden;
    -webkit-appearance: none;
    -moz-appearance: none;
    appearance: none;
    background: none;
    padding: 0;
  }

  /* 针对 WebKit 浏览器的颜色选择器内部样式 */
  .color-picker::-webkit-color-swatch-wrapper {
    padding: 0;
    border: none;
    border-radius: 50%;
    overflow: hidden;
  }

  .color-picker::-webkit-color-swatch {
    border: none;
    border-radius: 50%;
    box-shadow: none;
  }

  /* 针对 Firefox 浏览器的颜色选择器内部样式 */
  .color-picker::-moz-color-swatch {
    border: none;
    border-radius: 50%;
  }

  .color-picker::before {
    content: '';
    position: absolute;
    top: -2px;
    left: -2px;
    right: -2px;
    bottom: -2px;
    border-radius: 50%;
    background: linear-gradient(45deg, rgba(255, 255, 255, 0.8), transparent, rgba(255, 255, 255, 0.8));
    z-index: -1;
    opacity: 0;
    transition: opacity 0.3s ease;
  }

  .color-picker:hover {
    transform: scale(1.1);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.25);
  }

  .color-picker:hover::before {
    opacity: 1;
  }

  .color-text-input {
    flex: 1;
    padding: 14px 20px;
    border: 2px solid transparent;
    border-radius: 16px;
    font-size: 14px;
    font-family: 'SF Mono', 'Monaco', 'Consolas', monospace;
    background: linear-gradient(135deg, #f8fafc 0%, #f1f5f9 100%);
    color: #374151;
    font-weight: 500;
    transition: all 0.3s ease;
    box-shadow: inset 0 2px 4px rgba(0, 0, 0, 0.06);
  }

  .color-text-input:focus {
    outline: none;
    border-color: var(--theme-primary);
    background: white;
    box-shadow: 0 0 0 4px rgba(108, 122, 224, 0.1), inset 0 2px 4px rgba(0, 0, 0, 0.06);
    transform: translateY(-1px);
  }

  .color-text-input::placeholder {
    color: #9ca3af;
    font-style: italic;
  }

  .theme-name-input {
    padding: 16px 20px;
    border: 2px solid transparent;
    border-radius: 16px;
    font-size: 15px;
    font-weight: 500;
    background: linear-gradient(135deg, #f8fafc 0%, #f1f5f9 100%);
    color: #374151;
    transition: all 0.3s ease;
    box-shadow: inset 0 2px 4px rgba(0, 0, 0, 0.06);
  }

  .theme-name-input:focus {
    outline: none;
    border-color: var(--theme-primary);
    background: white;
    box-shadow: 0 0 0 4px rgba(108, 122, 224, 0.1), inset 0 2px 4px rgba(0, 0, 0, 0.06);
    transform: translateY(-1px);
  }

  .theme-name-input::placeholder {
    color: #9ca3af;
    font-style: italic;
  }

  .dialog-footer {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px 20px;
    border-top: 1px solid rgba(0, 0, 0, 0.06);
    background: linear-gradient(135deg, #f8fafc 0%, #f1f5f9 100%);
    position: relative;
    flex-shrink: 0;
    margin-top: auto;
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

  .reset-btn {
    padding: 8px 16px;
    border: none;
    border-radius: 10px;
    background: linear-gradient(135deg, #6b7280 0%, #4b5563 100%);
    color: white;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.3s ease;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    position: relative;
    overflow: hidden;
    box-shadow: 0 4px 12px rgba(107, 114, 128, 0.3);
  }

  .reset-btn::before {
    content: '';
    position: absolute;
    top: 0;
    left: -100%;
    width: 100%;
    height: 100%;
    background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.3), transparent);
    transition: left 0.5s ease;
  }

  .reset-btn:hover::before {
    left: 100%;
  }

  .action-buttons {
    display: flex;
    gap: 12px;
  }

  .dialog-footer .cancel-btn {
    padding: 8px 20px;
    border: none;
    border-radius: 10px;
    background: linear-gradient(135deg, #6b7280 0%, #4b5563 100%);
    color: white;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.3s ease;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    position: relative;
    overflow: hidden;
    box-shadow: 0 4px 12px rgba(107, 114, 128, 0.3);
  }

  .dialog-footer .cancel-btn::before {
    content: '';
    position: absolute;
    top: 0;
    left: -100%;
    width: 100%;
    height: 100%;
    background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.3), transparent);
    transition: left 0.5s ease;
  }

  .dialog-footer .cancel-btn:hover::before {
    left: 100%;
  }

  .apply-btn {
    padding: 8px 20px;
    border: none;
    border-radius: 10px;
    background: linear-gradient(135deg, var(--theme-primary) 0%, var(--theme-secondary) 100%);
    color: white;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.3s ease;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    position: relative;
    overflow: hidden;
    box-shadow: 0 4px 12px rgba(108, 122, 224, 0.3);
  }

  .apply-btn::before {
    content: '';
    position: absolute;
    top: 0;
    left: -100%;
    width: 100%;
    height: 100%;
    background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.3), transparent);
    transition: left 0.5s ease;
  }

  .apply-btn:hover::before {
    left: 100%;
  }

  /* 工具栏自定义主题按钮样式 */
  .custom-theme-btn {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    color: rgba(255, 255, 255, 0.8);
  }

  .custom-theme-btn svg {
    filter: drop-shadow(0 1px 2px rgba(0, 0, 0, 0.3));
  }

  .custom-theme-btn:hover svg {
    color: white;
  }

  /* 联系人加载状态样式 */
  .loading-contacts {
    text-align: center;
    padding: 32px 16px;
    color: var(--text-secondary);
  }

  .loading-spinner {
    width: 24px;
    height: 24px;
    border: 2px solid var(--border-color);
    border-top: 2px solid var(--accent-color);
    border-radius: 50%;
    animation: spin 1s linear infinite;
    margin: 0 auto 12px;
  }

  @keyframes spin {
    0% { transform: rotate(0deg); }
    100% { transform: rotate(360deg); }
  }

  /* 联系人列表容器 */
  .contacts-list {
    background: rgba(255, 255, 255, 0.8);
    backdrop-filter: blur(10px);
    border-radius: 12px;
    padding: 4px;
    border: 1px solid rgba(255, 255, 255, 0.2);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
  }

  /* 联系人项目样式 */
  .contact-item {
    display: flex;
    align-items: flex-start;
    padding: 2px 8px;
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.6);
    border: 1px solid rgba(255, 255, 255, 0.3);
  }

  .contact-item:last-child {
    margin-bottom: 0;
  }

  .contact-avatar {
    position: relative;
    width: 36px;
    height: 36px;
    margin-right: 10px;
    margin-top: 2px;
    flex-shrink: 0;
  }



  .contact-avatar-placeholder {
    width: 100%;
    height: 100%;
    border-radius: 50%;
    background: linear-gradient(135deg, #6c7ae0 0%, #8b5cf6 100%);
    display: flex;
    align-items: center;
    justify-content: center;
    color: white;
    font-weight: 600;
    font-size: 14px;
  }

  .contact-favorite-badge {
    position: absolute;
    top: -2px;
    right: -2px;
    width: 16px;
    height: 16px;
    background: #fbbf24;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    color: white;
    font-size: 8px;
  }

  .contact-info {
    flex: 1;
    min-width: 0;
  }

  .contact-name {
    font-weight: 600;
    color: var(--text-primary);
    font-size: 14px;
    margin-bottom: 2px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .contact-email {
    color: var(--text-secondary);
    font-size: 12px;
    margin-bottom: 2px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .contact-company {
    color: var(--text-tertiary);
    font-size: 11px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .contact-actions {
    display: flex;
    gap: 4px;
  }

  .contact-action-btn {
    width: 28px;
    height: 28px;
    border: none;
    border-radius: 6px;
    background: rgba(243, 244, 246, 0.8);
    color: #6b7280;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .contact-action-btn:hover {
    background: #6c7ae0;
    color: white;
  }


</style>

<!-- 添加联系人对话框 -->
<AddContactDialog
  bind:isOpen={showAddContactDialog}
  currentUser={$currentUser}
  on:contactAdded={handleContactAdded}
  on:close={() => showAddContactDialog = false}
/>
