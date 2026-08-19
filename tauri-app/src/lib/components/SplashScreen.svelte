<script lang="ts">
  import { onMount } from 'svelte';
  import { fade } from 'svelte/transition';

  export let onComplete = () => {};

  let showAnimation = false;

  onMount(() => {
    // 立即显示动画，减少透明窗口时间
    showAnimation = true;

    // 加载保存的主题设置并应用到开屏背景
    try {
      const savedTheme = localStorage.getItem('selectedTheme');
      if (savedTheme) {
        updateSplashTheme(savedTheme);
      }
    } catch (error) {
      console.warn('加载开屏主题失败:', error);
    }

    // 动画完成后延迟一段时间再结束
    setTimeout(() => {
      onComplete();
    }, 5500); // 稍微缩短总时间
  });

  // 更新开屏主题
  function updateSplashTheme(themeName: string) {
    const themes: Record<string, { primary: string; secondary: string }> = {
      default: { primary: '#6c7ae0', secondary: '#7b68ee' },
      sunset: { primary: '#d4a574', secondary: '#e6c2a6' },
      warm: { primary: '#fb7185', secondary: '#fda4af' },
      rose: { primary: '#f472b6', secondary: '#f9a8d4' },
      coral: { primary: '#8b5cf6', secondary: '#a78bfa' }
    };

    const theme = themes[themeName] || themes.default;
    const splashContainer = document.querySelector('.splash-container') as HTMLElement;
    if (splashContainer) {
      splashContainer.style.background = `linear-gradient(135deg, ${theme.primary}, ${theme.secondary})`;
    }
  }

</script>

<div class="splash-container">
  <!-- 背景渐变 -->
  <div class="background-gradient"></div>

  <!-- 装饰性几何图形 -->
  <div class="geometric-shapes">
    <div class="shape shape-1"></div>
    <div class="shape shape-2"></div>
    <div class="shape shape-3"></div>
    <div class="shape shape-4"></div>
  </div>

  <!-- Apple Hello 动画 -->
  <div class="content-wrapper">
    {#if showAnimation}
      <div class="hello-animation" in:fade={{ duration: 500 }}>
        <svg
          class="hello-svg"
          xmlns="http://www.w3.org/2000/svg"
          viewBox="0 0 638 200"
          fill="none"
          stroke="currentColor"
          stroke-width="14.8883"
        >

          <!-- h1 -->
          <path
            class="path-1"
            d="M8.69214 166.553C36.2393 151.239 61.3409 131.548 89.8191 98.0295C109.203 75.1488 119.625 49.0228 120.122 31.0026C120.37 17.6036 113.836 7.43883 101.759 7.43883C88.3598 7.43883 79.9231 17.6036 74.7122 40.9363C69.005 66.5793 64.7866 96.0036 54.1166 190.356"
            style="stroke-linecap: round"
          />

          <!-- h2, ello -->
          <path
            class="path-2"
            d="M55.1624 181.135C60.6251 133.114 81.4118 98.0479 107.963 98.0479C123.844 98.0479 133.937 110.703 131.071 128.817C129.457 139.487 127.587 150.405 125.408 163.06C122.869 178.941 130.128 191.348 152.122 191.348C184.197 191.348 219.189 173.523 237.097 145.915C243.198 136.509 245.68 128.073 245.928 119.884C246.176 104.996 237.739 93.8296 222.851 93.8296C203.992 93.8296 189.6 115.17 189.6 142.465C189.6 171.745 205.481 192.341 239.208 192.341C285.066 192.341 335.86 137.292 359.199 75.8585C365.788 58.513 368.26 42.4065 368.26 31.1512C368.26 17.8057 364.042 7.55823 352.131 7.55823C340.469 7.55823 332.777 16.6141 325.829 30.9129C317.688 47.4967 311.667 71.4162 309.203 98.4549C303 166.301 316.896 191.348 349.936 191.348C390 191.348 434.542 135.534 457.286 75.6686C463.803 58.513 466.275 42.4065 466.275 31.1512C466.275 17.8057 462.057 7.55823 450.146 7.55823C438.484 7.55823 430.792 16.6141 423.844 30.9129C415.703 47.4967 409.682 71.4162 407.218 98.4549C401.015 166.301 414.911 191.348 444.416 191.348C473.874 191.348 489.877 165.67 499.471 138.402C508.955 111.447 520.618 94.8221 544.935 94.8221C565.035 94.8221 580.916 109.71 580.916 137.75C580.916 168.768 560.792 192.093 535.362 192.341C512.984 192.589 498.285 174.475 499.774 147.179C501.511 116.907 519.873 94.8221 543.943 94.8221C557.839 94.8221 569.51 100.999 578.682 107.725C603.549 125.866 622.709 114.656 630.047 96.7186"
            style="stroke-linecap: round"
          />
        </svg>
      </div>
    {/if}
  </div>

  <!-- 加载指示器 -->
  <div class="loading-indicator">
    <div class="loading-dots">
      <div class="dot"></div>
      <div class="dot"></div>
      <div class="dot"></div>
    </div>
  </div>
</div>

<style>
  .splash-container {
    position: fixed;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100vh;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    z-index: 9999;
    overflow: hidden;
    /* 启动动画背景 - 默认主题色，会被JavaScript动态更新 */
    background: linear-gradient(135deg, #6c7ae0, #7b68ee);
    /* 性能优化 */
    transform: translateZ(0);
    backface-visibility: hidden;
    perspective: 1000px;

  }
  
  /* 背景渐变现在直接在容器上，这个元素不再需要 */
  .background-gradient {
    display: none;
  }
  
  /* 装饰性几何图形 */
  .geometric-shapes {
    position: absolute;
    width: 100%;
    height: 100%;
    pointer-events: none;
  }
  
  .shape {
    position: absolute;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.08);
    will-change: transform;
    animation: floatOptimized 12s ease-in-out infinite;
  }

  .shape-1 {
    width: 120px;
    height: 120px;
    top: 10%;
    left: 10%;
    animation-delay: 0s;
  }

  .shape-2 {
    width: 80px;
    height: 80px;
    top: 20%;
    right: 15%;
    animation-delay: 3s;
  }

  .shape-3 {
    width: 100px;
    height: 100px;
    bottom: 20%;
    left: 20%;
    animation-delay: 6s;
  }

  .shape-4 {
    width: 60px;
    height: 60px;
    bottom: 15%;
    right: 25%;
    animation-delay: 9s;
  }

  @keyframes floatOptimized {
    0%, 100% {
      transform: translate3d(0, 0, 0) rotate(0deg);
    }
    25% {
      transform: translate3d(10px, -15px, 0) rotate(90deg);
    }
    50% {
      transform: translate3d(0, -20px, 0) rotate(180deg);
    }
    75% {
      transform: translate3d(-10px, -15px, 0) rotate(270deg);
    }
  }
  
  /* 主要内容区域 */
  .content-wrapper {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2rem;
    z-index: 10;
  }
  
  /* Apple Hello 动画部分 */
  .hello-animation {
    display: flex;
    justify-content: center;
    align-items: center;
    height: 200px;
  }

  .hello-svg {
    height: 120px;
    width: auto;
    color: white;
    filter: drop-shadow(0 4px 8px rgba(0, 0, 0, 0.3));
    will-change: auto;
    transform: translateZ(0);
  }

  .path-1 {
    stroke-dasharray: 1000;
    stroke-dashoffset: 1000;
    animation: drawPath1 0.8s cubic-bezier(0.4, 0, 0.2, 1) forwards;
    will-change: stroke-dashoffset;
  }

  .path-2 {
    stroke-dasharray: 3000;
    stroke-dashoffset: 3000;
    animation: drawPath2 2.8s cubic-bezier(0.4, 0, 0.2, 1) 0.7s forwards;
    will-change: stroke-dashoffset;
  }

  @keyframes drawPath1 {
    to {
      stroke-dashoffset: 0;
    }
  }

  @keyframes drawPath2 {
    to {
      stroke-dashoffset: 0;
    }
  }
  

  
  /* 加载指示器 */
  .loading-indicator {
    position: absolute;
    bottom: 3rem;
    left: 50%;
    transform: translateX(-50%);
  }
  
  .loading-dots {
    display: flex;
    gap: 0.5rem;
  }
  
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.7);
    will-change: transform;
    animation: pulseOptimized 1.5s ease-in-out infinite;
  }

  .dot:nth-child(1) { animation-delay: 0s; }
  .dot:nth-child(2) { animation-delay: 0.3s; }
  .dot:nth-child(3) { animation-delay: 0.6s; }

  @keyframes pulseOptimized {
    0%, 100% {
      transform: scale3d(1, 1, 1);
      opacity: 0.3;
    }
    50% {
      transform: scale3d(1.2, 1.2, 1);
      opacity: 1;
    }
  }
  
  /* 响应式设计 */
  @media (max-width: 768px) {
    .hello-svg {
      height: 80px;
    }
  }
</style>
