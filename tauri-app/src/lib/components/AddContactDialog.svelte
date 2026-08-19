<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { EmailAPI } from '../api';
  import type { NewContact } from '../types';
  
  export let isOpen = false;
  export let currentUser: any = null;
  
  const dispatch = createEventDispatcher();
  
  let name = '';
  let email = '';
  let phone = '';
  let company = '';
  let notes = '';
  let isFavorite = false;
  let isLoading = false;
  let errorMessage = '';
  
  function closeDialog() {
    isOpen = false;
    resetForm();
    dispatch('close');
  }
  
  function resetForm() {
    name = '';
    email = '';
    phone = '';
    company = '';
    notes = '';
    isFavorite = false;
    errorMessage = '';
  }
  
  async function handleSubmit() {
    if (!name.trim() || !email.trim()) {
      errorMessage = '姓名和邮箱是必填项';
      return;
    }
    
    if (!currentUser?.id) {
      errorMessage = '用户信息无效';
      return;
    }
    
    isLoading = true;
    errorMessage = '';
    
    try {
      const contactData: NewContact = {
        user_id: currentUser.id,
        name: name.trim(),
        email: email.trim(),
        phone: phone.trim() || undefined,
        company: company.trim() || undefined,
        notes: notes.trim() || undefined,
        avatar: undefined,
        is_favorite: isFavorite
      };
      
      await EmailAPI.createContact(contactData);
      dispatch('contactAdded');
      closeDialog();
    } catch (error) {
      console.error('添加联系人失败:', error);
      errorMessage = error?.toString() || '添加联系人失败';
    } finally {
      isLoading = false;
    }
  }
  
  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      closeDialog();
    } else if (event.key === 'Enter' && (event.ctrlKey || event.metaKey)) {
      handleSubmit();
    }
  }


</script>

<svelte:window on:keydown={handleKeydown} />

{#if isOpen}
  <!-- svelte-ignore a11y-click-events-have-key-events -->
  <!-- svelte-ignore a11y-no-static-element-interactions -->
  <div class="dialog-overlay" role="dialog" aria-modal="true" tabindex="-1" on:click={closeDialog} on:keydown={handleKeydown}>
    <!-- svelte-ignore a11y-click-events-have-key-events -->
    <!-- svelte-ignore a11y-no-static-element-interactions -->
    <div class="dialog-container" on:click|stopPropagation>
      <div class="dialog-header">
        <h3>添加联系人</h3>
      </div>
      
      <div class="dialog-content">
        {#if errorMessage}
          <div class="error-message">
            {errorMessage}
          </div>
        {/if}
        
        <form on:submit|preventDefault={handleSubmit}>
          <div class="form-group">
            <label for="name">姓名 *</label>
            <input
              id="name"
              type="text"
              bind:value={name}
              placeholder="请输入姓名"
              required
              disabled={isLoading}
            />
          </div>
          
          <div class="form-group">
            <label for="email">邮箱 *</label>
            <input
              id="email"
              type="email"
              bind:value={email}
              placeholder="请输入邮箱地址"
              required
              disabled={isLoading}
            />
          </div>
          
          <div class="form-group">
            <label for="phone">电话</label>
            <input
              id="phone"
              type="tel"
              bind:value={phone}
              placeholder="请输入电话号码"
              disabled={isLoading}
            />
          </div>
          
          <div class="form-group">
            <label for="company">公司</label>
            <input
              id="company"
              type="text"
              bind:value={company}
              placeholder="请输入公司名称"
              disabled={isLoading}
            />
          </div>
          
          <div class="form-group">
            <label for="notes">备注</label>
            <textarea
              id="notes"
              bind:value={notes}
              placeholder="请输入备注信息"
              rows="3"
              disabled={isLoading}
            ></textarea>
          </div>
          
          <div class="form-group checkbox-group">
            <label class="checkbox-label">
              <input
                type="checkbox"
                bind:checked={isFavorite}
                disabled={isLoading}
              />
              <span class="checkmark"></span>
              设为收藏联系人
            </label>
          </div>
        </form>
      </div>
      
      <div class="dialog-actions">
        <button
          type="button"
          class="btn btn-secondary"
          on:click={closeDialog}
          disabled={isLoading}
        >
          取消
        </button>
        <button
          type="button"
          class="btn btn-primary"
          on:click={handleSubmit}
          disabled={isLoading || !name.trim() || !email.trim()}
        >
          {#if isLoading}
            添加中...
          {:else}
            添加联系人
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

  .dialog-container {
    background: linear-gradient(135deg, #ffffff 0%, #f8f9fa 100%);
    border-radius: 20px;
    width: 90%;
    max-width: 500px;
    max-height: 90vh;
    overflow: hidden;
    box-shadow:
      0 25px 50px rgba(0, 0, 0, 0.15),
      0 10px 20px rgba(0, 0, 0, 0.1),
      0 0 0 1px rgba(255, 255, 255, 0.8);
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

  .dialog-header h3 {
    margin: 0;
    color: var(--theme-primary);
    font-size: 16px;
    font-weight: 600;
  }

  .dialog-content {
    padding: 20px 24px;
    flex: 1;
    overflow-y: auto;
    background: linear-gradient(135deg, #fafbfc 0%, #f8f9fa 100%);
    min-height: 0;
  }
  
  .error-message {
    background: rgba(239, 68, 68, 0.1);
    border: 1px solid rgba(239, 68, 68, 0.3);
    color: #ef4444;
    padding: 12px;
    border-radius: 8px;
    margin-bottom: 20px;
    font-size: 14px;
  }
  
  .form-group {
    margin-bottom: 20px;
  }
  
  .form-group label {
    display: block;
    margin-bottom: 8px;
    font-weight: 500;
    color: #374151;
    font-size: 14px;
  }

  .form-group input,
  .form-group textarea {
    width: 100%;
    padding: 12px 16px;
    border: 2px solid #e5e7eb;
    border-radius: 12px;
    background: white;
    color: #374151;
    font-size: 14px;
    transition: all 0.3s ease;
    box-sizing: border-box;
    font-family: inherit;
    line-height: 1.5;
  }

  .form-group input:focus,
  .form-group textarea:focus {
    outline: none;
    border-color: var(--theme-primary);
    box-shadow: 0 0 0 4px rgba(102, 126, 234, 0.1);
    transform: translateY(-1px);
  }

  .form-group input:hover,
  .form-group textarea:hover {
    border-color: #d1d5db;
  }

  .form-group input::placeholder,
  .form-group textarea::placeholder {
    color: #9ca3af;
    font-style: italic;
  }

  .form-group textarea {
    resize: vertical;
    min-height: 80px;
    max-height: 120px;
  }
  
  .form-group input:disabled,
  .form-group textarea:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
  
  .checkbox-group {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 24px;
    padding: 16px;
    background: rgba(102, 126, 234, 0.05);
    border-radius: 12px;
    border: 1px solid rgba(102, 126, 234, 0.1);
  }

  .checkbox-label {
    display: flex;
    align-items: center;
    cursor: pointer;
    font-size: 14px;
    color: #374151;
    font-weight: 500;
    user-select: none;
  }

  .checkbox-label input[type="checkbox"] {
    width: 18px;
    height: 18px;
    margin-right: 8px;
    accent-color: var(--theme-primary);
    cursor: pointer;
  }
  
  .dialog-actions {
    display: flex;
    justify-content: flex-end;
    gap: 12px;
    padding: 12px 20px;
    border-top: 1px solid rgba(0, 0, 0, 0.06);
    background: linear-gradient(135deg, #f8fafc 0%, #f1f5f9 100%);
    flex-shrink: 0;
    position: relative;
  }

  .dialog-actions::before {
    content: '';
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 1px;
    background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.8), transparent);
  }

  .btn {
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
    min-width: 100px;
  }

  .btn::before {
    content: '';
    position: absolute;
    top: 0;
    left: -100%;
    width: 100%;
    height: 100%;
    background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.3), transparent);
    transition: left 0.5s ease;
  }

  .btn:hover::before {
    left: 100%;
  }

  .btn-secondary {
    background: linear-gradient(135deg, #6b7280 0%, #4b5563 100%);
    color: white;
    box-shadow: 0 4px 12px rgba(107, 114, 128, 0.3);
  }

  .btn-primary {
    background: linear-gradient(135deg, var(--theme-primary) 0%, var(--theme-secondary) 100%);
    color: white;
    box-shadow: 0 4px 12px rgba(108, 122, 224, 0.3);
  }

  .btn-primary:disabled {
    background: linear-gradient(135deg, #9ca3af 0%, #6b7280 100%);
    cursor: not-allowed;
    box-shadow: 0 4px 12px rgba(107, 114, 128, 0.3);
  }

  .btn-primary:disabled::before {
    display: none;
  }

  /* 自定义滚动条样式 - 与其他对话框保持一致 */
  .dialog-content::-webkit-scrollbar {
    width: 8px;
  }

  .dialog-content::-webkit-scrollbar-track {
    background: #f1f5f9;
    border-radius: 4px;
  }

  .dialog-content::-webkit-scrollbar-thumb {
    background: linear-gradient(135deg, #cbd5e1 0%, #94a3b8 100%);
    border-radius: 4px;
    transition: background 0.3s ease;
  }

  .dialog-content::-webkit-scrollbar-thumb:hover {
    background: linear-gradient(135deg, #94a3b8 0%, #64748b 100%);
  }

  /* 移除滚动条箭头按钮 */
  .dialog-content::-webkit-scrollbar-button {
    display: none;
  }

  /* Firefox 滚动条样式 */
  .dialog-content {
    scrollbar-width: thin;
    scrollbar-color: #cbd5e1 #f1f5f9;
  }
</style>
