<script lang="ts">
  export let currentStep = 0;
  export let totalSteps = 4; // 保持API兼容性
  export let isVisible = true;

  // 避免未使用警告
  totalSteps;

  // 简单的步骤文本
  const steps = [
    "准备中...",
    "连接邮箱服务器",
    "获取邮件列表",
    "整理邮件内容",
    "完成加载"
  ];

  // 获取当前步骤文本
  $: stepText = steps[currentStep] || "准备中...";

  // totalSteps参数保持兼容性，当前使用固定步骤数
  // 可以在未来扩展为动态步骤数
</script>

{#if isVisible}
  <div class="loading-wrapper">
    <div class="loading-icon">📧</div>
    <div class="loading-text">{stepText}</div>
  </div>
{/if}

<style>
  .loading-wrapper {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 3rem 2rem;
    max-width: 400px;
    margin: 0 auto;
    text-align: center;
  }

  .loading-icon {
    font-size: 3rem;
    margin-bottom: 1rem;
    opacity: 0.8;
  }

  .loading-text {
    font-size: 0.9rem;
    color: #6b7280;
    margin-top: 1rem;
    min-height: 1.2rem;
  }
</style>