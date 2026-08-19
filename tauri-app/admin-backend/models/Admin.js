const bcrypt = require('bcryptjs');

class Admin {
  constructor(database) {
    this.db = database;
  }

  // 创建管理员
  async createAdmin(username, password) {
    const hashedPassword = await bcrypt.hash(password, 10);
    const sql = 'INSERT INTO admin_users (username, password) VALUES (?, ?)';
    return await this.db.query(sql, [username, hashedPassword]);
  }

  // 根据用户名查找管理员
  async findByUsername(username) {
    const sql = 'SELECT * FROM admin_users WHERE username = ?';
    const result = await this.db.query(sql, [username]);
    return result[0] || null;
  }

  // 验证密码
  async verifyPassword(plainPassword, hashedPassword) {
    return await bcrypt.compare(plainPassword, hashedPassword);
  }

  // 登录验证
  async login(username, password) {
    const admin = await this.findByUsername(username);
    if (!admin) {
      return { success: false, message: '用户名不存在' };
    }

    const isValidPassword = await this.verifyPassword(password, admin.password);
    if (!isValidPassword) {
      return { success: false, message: '密码错误' };
    }

    return { 
      success: true, 
      message: '登录成功',
      admin: {
        id: admin.id,
        username: admin.username,
        created_at: admin.created_at
      }
    };
  }

  // 初始化默认管理员
  async initDefaultAdmin() {
    const username = process.env.ADMIN_USERNAME;
    const password = process.env.ADMIN_PASSWORD;
    
    const existingAdmin = await this.findByUsername(username);
    if (!existingAdmin) {
      await this.createAdmin(username, password);
      console.log(`默认管理员创建成功: ${username}`);
    } else {
      console.log(`管理员已存在: ${username}`);
    }
  }
}

module.exports = Admin;
