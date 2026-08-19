// 头像工具函数 - 统一的头像获取逻辑

// 获取发件人显示名称
export function getSenderDisplayName(sender: string): string {
  // 处理空或无效的sender
  if (!sender || sender.trim() === '') {
    return '我';  // 草稿邮件通常是用户自己创建的
  }

  // 检查是否是 "Name <email>" 格式
  const match = sender.match(/^(.+?)\s*<.+>$/);
  if (match) {
    return match[1].trim();
  }

  // 如果只是邮箱地址，提取@前面的部分作为显示名称
  if (sender.includes('@')) {
    const atIndex = sender.indexOf('@');
    return sender.substring(0, atIndex);
  }

  return sender;
}

// 获取发件人邮箱地址
export function getSenderEmail(sender: string): string {
  // 处理空或无效的sender
  if (!sender || sender.trim() === '') {
    return '';  // 返回空字符串，让调用方处理
  }

  // 检查是否是 "Name <email>" 格式
  const match = sender.match(/<(.+)>/);
  if (match) {
    return match[1];
  }

  // 如果直接是邮箱地址
  if (sender.includes('@')) {
    return sender;
  }

  return sender;
}

// 简单哈希函数
function simpleHash(str: string): string {
  let hash = 0;
  for (let i = 0; i < str.length; i++) {
    const char = str.charCodeAt(i);
    hash = ((hash << 5) - hash) + char;
    hash = hash & hash; // 转换为32位整数
  }
  return Math.abs(hash).toString(16).padStart(8, '0');
}

// 智能服务识别 - 根据发件人名称精确匹配（使用本地头像）
function getServiceSpecificLogo(sender: string, email: string): string | null {
  const senderName = getSenderDisplayName(sender).toLowerCase();
  const emailLower = email.toLowerCase();

  // 云服务和技术服务（优先匹配，避免被华为误识别）
  if (senderName.includes('cloudns') || emailLower.includes('cloudns.net')) {
    return '/src/assets/youjiantubiao/clouDNS.png';
  }
  if (senderName.includes('cloudflare') || emailLower.includes('cloudflare.com')) {
    return '/src/assets/youjiantubiao/cloudflare.png';
  }

  // 华为系服务（更精确的匹配）
  if (senderName.includes('华为云') || senderName.includes('huawei cloud') || emailLower.includes('huaweicloud.com')) {
    return '/src/assets/youjiantubiao/huawei.png';
  }
  if (senderName.includes('华为') || senderName.includes('huawei')) {
    return '/src/assets/youjiantubiao/huawei.png';
  }

  // 腾讯系服务
  if (senderName.includes('腾讯视频') || senderName.includes('tencent video')) {
    return '/src/assets/youjiantubiao/tenxunshipin.png';
  }
  if (senderName.includes('微信') || senderName.includes('wechat')) {
    return '/src/assets/youjiantubiao/weixin.png';
  }
  if (senderName.includes('腾讯云') || senderName.includes('tencent cloud')) {
    return '/src/assets/youjiantubiao/tenxunyun.png';
  }
  if (senderName.includes('腾讯') || senderName.includes('tencent')) {
    return '/src/assets/youjiantubiao/QQ.png'; // 使用QQ图标作为腾讯的通用图标
  }
  if (senderName.includes('qq') || emailLower.includes('qq.com')) {
    return '/src/assets/youjiantubiao/QQ.png';
  }

  // 阿里系服务
  if (senderName.includes('支付宝') || senderName.includes('alipay')) {
    return '/src/assets/youjiantubiao/zhifubao.png';
  }
  if (senderName.includes('淘宝') || senderName.includes('taobao')) {
    return '/src/assets/youjiantubiao/taobao.png';
  }
  if (senderName.includes('天猫') || senderName.includes('tmall')) {
    return '/src/assets/youjiantubiao/tianmao.png';
  }
  if (senderName.includes('阿里云') || senderName.includes('aliyun')) {
    return '/src/assets/youjiantubiao/aliyun.png';
  }
  if (senderName.includes('阿里') || senderName.includes('alibaba')) {
    return '/src/assets/youjiantubiao/aliyun.png';
  }

  // 百度系服务
  if (senderName.includes('百度') || senderName.includes('baidu')) {
    return '/src/assets/youjiantubiao/baidu.png';
  }

  // 小米系服务
  if (senderName.includes('小米') || senderName.includes('xiaomi') || senderName.includes('mi.com')) {
    return '/src/assets/youjiantubiao/xiaom.png';
  }

  // 字节跳动系服务
  if (senderName.includes('抖音') || senderName.includes('douyin')) {
    return '/src/assets/youjiantubiao/douyin.png';
  }
  if (senderName.includes('今日头条') || senderName.includes('toutiao')) {
    return '/src/assets/youjiantubiao/jinritotiao.png';
  }
  if (senderName.includes('字节跳动') || senderName.includes('bytedance')) {
    return '/src/assets/youjiantubiao/zijietiaodon.png';
  }

  // 网易系服务
  if (senderName.includes('网易云音乐') || senderName.includes('netease music')) {
    return '/src/assets/youjiantubiao/wangyiyunyinyue.png';
  }
  if (senderName.includes('网易') || senderName.includes('netease')) {
    return '/src/assets/youjiantubiao/wangyiyunyinyue.png';
  }

  // 美团系服务
  if (senderName.includes('美团') || senderName.includes('meituan')) {
    return '/src/assets/youjiantubiao/meituan.png';
  }
  if (senderName.includes('大众点评') || senderName.includes('dianping')) {
    return '/src/assets/youjiantubiao/dazhongdianping.png';
  }

  // 滴滴系服务
  if (senderName.includes('滴滴') || senderName.includes('didi')) {
    return '/src/assets/youjiantubiao/didi.png';
  }

  // B站
  if (senderName.includes('哔哩哔哩') || senderName.includes('bilibili') || senderName.includes('b站')) {
    return '/src/assets/youjiantubiao/blibli.png';
  }

  // 知乎
  if (senderName.includes('知乎') || senderName.includes('zhihu')) {
    return '/src/assets/youjiantubiao/zhihu.png';
  }

  // 银行系统
  // 四大国有银行
  if (senderName.includes('中国工商银行') || senderName.includes('工商银行') || senderName.includes('icbc')) {
    return '/src/assets/youjiantubiao/goshangyinhan.png';
  }
  if (senderName.includes('中国建设银行') || senderName.includes('建设银行') || senderName.includes('ccb')) {
    return '/src/assets/youjiantubiao/zhongguojiansheyinhang.png';
  }
  if (senderName.includes('中国农业银行') || senderName.includes('农业银行') || senderName.includes('abc')) {
    return '/src/assets/youjiantubiao/nonyeyinhang.png';
  }
  if (senderName.includes('中国银行') || senderName.includes('boc')) {
    return '/src/assets/youjiantubiao/zhonguoyinhang.png';
  }

  // 股份制银行
  if (senderName.includes('招商银行') || senderName.includes('cmb')) {
    return '/src/assets/youjiantubiao/zhaoshangyinhang.png';
  }
  if (senderName.includes('中信银行') || senderName.includes('citic')) {
    return '/src/assets/youjiantubiao/zhonxinyinhang.png';
  }
  if (senderName.includes('平安银行') || senderName.includes('pingan bank')) {
    return '/src/assets/youjiantubiao/pinganyinhang.png';
  }
  if (senderName.includes('广发银行') || senderName.includes('cgb')) {
    return '/src/assets/youjiantubiao/guangfayinhang.png';
  }



  // 其他常见服务
  if (senderName.includes('github') || emailLower.includes('github')) {
    return '/src/assets/youjiantubiao/github.png';
  }
  if (senderName.includes('gmail') || emailLower.includes('gmail')) {
    return '/src/assets/youjiantubiao/gmail.png';
  }
  if (senderName.includes('微博') || senderName.includes('weibo')) {
    return '/src/assets/youjiantubiao/weibo.png';
  }
  if (senderName.includes('豆瓣') || senderName.includes('douban')) {
    return '/src/assets/youjiantubiao/douban.png';
  }
  if (senderName.includes('快手') || senderName.includes('kuaishou')) {
    return '/src/assets/youjiantubiao/kuaishou.png';
  }
  if (senderName.includes('饿了么') || senderName.includes('eleme')) {
    return '/src/assets/youjiantubiao/elme.png';
  }
  if (senderName.includes('高德地图') || senderName.includes('gaode')) {
    return '/src/assets/youjiantubiao/gaodeditu.png';
  }

  return null;
}

// 获取企业Logo URL
function getCompanyLogoUrl(domain: string): string {
  // 使用Clearbit Logo API
  return `https://logo.clearbit.com/${domain}?size=80`;
}

// 获取Gravatar头像URL
function getGravatarUrl(email: string): string {
  const emailLower = email.toLowerCase().trim();
  const hash = simpleHash(emailLower);
  return `https://www.gravatar.com/avatar/${hash}?s=80&d=404`;
}

// 头像缓存 - 改进版本，包含时间戳和重试机制
interface CacheEntry {
  success: boolean;
  timestamp: number;
  retryCount: number;
}

const avatarCache = new Map<string, CacheEntry>();
const CACHE_EXPIRE_TIME = 5 * 60 * 1000; // 5分钟过期
const MAX_RETRY_COUNT = 2; // 最多重试2次
const RETRY_DELAY = 30 * 1000; // 30秒后可以重试

// 检查图片是否存在（改进版本）
async function checkImageExists(url: string): Promise<boolean> {
  const now = Date.now();
  const cached = avatarCache.get(url);

  // 检查缓存是否有效
  if (cached) {
    const isExpired = now - cached.timestamp > CACHE_EXPIRE_TIME;
    const canRetry = cached.retryCount < MAX_RETRY_COUNT &&
                     now - cached.timestamp > RETRY_DELAY;

    // 如果缓存成功且未过期，直接返回
    if (cached.success && !isExpired) {
      return true;
    }

    // 如果缓存失败但不能重试，返回失败
    if (!cached.success && !canRetry && !isExpired) {
      return false;
    }
  }

  // 需要重新检查图片
  return new Promise((resolve) => {
    const img = new Image();
    let resolved = false;

    img.onload = () => {
      if (!resolved) {
        resolved = true;
        avatarCache.set(url, {
          success: true,
          timestamp: now,
          retryCount: 0
        });
        resolve(true);
      }
    };

    img.onerror = () => {
      if (!resolved) {
        resolved = true;
        const retryCount = cached ? cached.retryCount + 1 : 1;
        avatarCache.set(url, {
          success: false,
          timestamp: now,
          retryCount
        });
        resolve(false);
      }
    };

    // 设置超时，但延长到5秒
    setTimeout(() => {
      if (!resolved) {
        resolved = true;
        const retryCount = cached ? cached.retryCount + 1 : 1;
        avatarCache.set(url, {
          success: false,
          timestamp: now,
          retryCount
        });
        resolve(false);
      }
    }, 5000);

    img.src = url;
  });
}

// 获取发件人头像信息（智能识别版本）
function getSenderAvatarInfo(sender: string) {
  const email = getSenderEmail(sender);
  const domain = email.split('@')[1]?.toLowerCase();

  if (!domain) return { hasLocalLogo: false, urls: [] };

  const urls = [];

  // 1. 智能服务识别（根据发件人名称）
  const specificLogo = getServiceSpecificLogo(sender, email);
  if (specificLogo) {
    urls.push(specificLogo);
  }

  // 2. 通用域名Logo（Clearbit）
  urls.push(getCompanyLogoUrl(domain));

  // 3. 个人头像（Gravatar）
  urls.push(getGravatarUrl(email));

  return { hasLocalLogo: false, urls };
}

// 清理过期的缓存条目
function cleanExpiredCache() {
  const now = Date.now();
  for (const [url, entry] of avatarCache.entries()) {
    if (now - entry.timestamp > CACHE_EXPIRE_TIME) {
      avatarCache.delete(url);
    }
  }
}

// 智能获取发件人头像URL（异步版本，用于缓存）
export async function getSenderAvatarUrl(sender: string): Promise<string | null> {
  // 处理空或无效的sender（草稿邮件的情况）
  if (!sender || sender.trim() === '') {
    return null;  // 返回null，让界面显示默认头像（用户名首字母）
  }

  // 定期清理过期缓存
  if (Math.random() < 0.1) { // 10%的概率清理缓存
    cleanExpiredCache();
  }

  // 获取邮箱地址用于头像查询
  const email = getSenderEmail(sender);
  if (!email || !email.includes('@')) {
    return null;  // 无效邮箱地址，返回null显示默认头像
  }

  const info = getSenderAvatarInfo(sender);

  // 依次尝试各个URL
  for (const url of info.urls) {
    if (await checkImageExists(url)) {
      return url;
    }
  }

  return null;
}

// 手动清理缓存的函数（供调试使用）
export function clearAvatarCache() {
  avatarCache.clear();
}
