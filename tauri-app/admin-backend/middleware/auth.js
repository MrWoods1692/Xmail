// 检查是否已登录
function requireAuth(req, res, next) {
  if (req.session && req.session.admin) {
    return next();
  } else {
    return res.status(401).json({ 
      success: false, 
      message: '请先登录',
      redirect: '/login'
    });
  }
}

// 检查是否已登录（页面重定向版本）
function requireAuthPage(req, res, next) {
  if (req.session && req.session.admin) {
    return next();
  } else {
    return res.redirect('/login');
  }
}

// 检查是否未登录（用于登录页面）
function requireGuest(req, res, next) {
  if (req.session && req.session.admin) {
    return res.redirect('/dashboard');
  } else {
    return next();
  }
}

module.exports = {
  requireAuth,
  requireAuthPage,
  requireGuest
};
