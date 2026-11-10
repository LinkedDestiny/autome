# 空输入检查

status: 已定位
fix-round: 1/3
复现命令: sh regression.sh
根因: check.sh 没有检查空参数
复核: 待复核

## 现象
空输入成功。
## 复现
sh regression.sh
## 根因分析
缺少空参数校验。
## 修复说明

## 复核意见
