<script lang="ts">
  import { notifications } from '../store';
  import { fly } from 'svelte/transition';

  function removeNotification(id: string) {
    notifications.update(items => items.filter(item => item.id !== id));
  }
</script>

<div class="notifications-container">
  {#each $notifications as notification (notification.id)}
    <div
      class="notification notification-{notification.type}"
      transition:fly={{ x: -300, duration: 400 }}
      role="button"
      tabindex="0"
      aria-label="点击关闭通知"
      on:click={() => removeNotification(notification.id)}
      on:keydown={(e) => e.key === 'Enter' && removeNotification(notification.id)}
    >
      <div class="notification-content">
        <span class="notification-message">{notification.message}</span>
      </div>
    </div>
  {/each}
</div>

<style>
  .notifications-container {
    position: fixed;
    bottom: 20px;
    left: 30px; /* 左边距30px */
    right: auto;
    z-index: 1001;
    display: flex;
    flex-direction: column;
    gap: 12px;
    max-width: 220px; /* 确保左右间距对等 */
    width: calc(280px - 60px); /* 左侧账号区域实际宽度280px减去左右边距（30px + 30px） */
  }

  .notification {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 10px 14px; /* 稍微减小内边距 */
    border-radius: 8px; /* 稍微减小圆角 */
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.15);
    cursor: pointer;
    transition: all 0.3s ease;
    backdrop-filter: blur(10px);
    border: 1px solid rgba(255, 255, 255, 0.2);
    outline: none;
    word-wrap: break-word; /* 确保长文本换行 */
    overflow-wrap: break-word;
  }

  .notification:hover {
    transform: translateX(5px);
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.2);
  }

  .notification:focus {
    transform: translateX(5px);
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.2);
  }

  .notification-success {
    background: linear-gradient(135deg, rgba(76, 175, 80, 0.9), rgba(56, 142, 60, 0.9));
    color: white;
    border-color: rgba(76, 175, 80, 0.3);
  }

  .notification-error {
    background: linear-gradient(135deg, rgba(244, 67, 54, 0.9), rgba(211, 47, 47, 0.9));
    color: white;
    border-color: rgba(244, 67, 54, 0.3);
  }

  .notification-warning {
    background: linear-gradient(135deg, rgba(255, 193, 7, 0.9), rgba(255, 160, 0, 0.9));
    color: white;
    border-color: rgba(255, 193, 7, 0.3);
  }

  .notification-info {
    background: linear-gradient(135deg, rgba(33, 150, 243, 0.9), rgba(30, 136, 229, 0.9));
    color: white;
    border-color: rgba(33, 150, 243, 0.3);
  }

  .notification-content {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
  }

  .notification-message {
    font-size: 13px; /* 稍微减小字体 */
    line-height: 1.3;
    font-weight: 500;
    text-align: center;
    word-break: break-word; /* 确保长文本正确换行 */
    hyphens: auto; /* 自动断词 */
  }
</style>
