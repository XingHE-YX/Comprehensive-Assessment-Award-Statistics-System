# 2026-09-10 HTTP 验收明细

受测提交：`7ad62cf6d1cef9310384d8f4f4f24a491d7d7acb`。实际 `linux/amd64` 应用镜像、SQLx SQLite `3.46.1`，独立临时数据库，真实 HTTP multipart。输入均为合成数据；没有记录密码、修改码、会话值或申报编号。

共 110 例，109 例符合预期，1 例不符合。F2 是已复现的非校级荣誉被错误要求填写校级字段，不能当作正确行为验收。各类别缺字段用例均从已独立提交成功的对应分支样例中逐个移除字段，避免多个错误互相掩盖。完整审查结论见 [最终验收报告](../final-acceptance.md)。

| 用例 | 预期 HTTP | 实际 HTTP | 结论 | 提示摘录 |
|---|---:|---:|---|---|
| academic_award.valid | 303 | 303 | 符合 | — |
| academic_award.missing.competition_name | 422 | 422 | 符合 | 竞赛完整名称为必填项 |
| academic_award.missing.competition_type | 422 | 422 | 符合 | 学科竞赛类别为必填项 |
| academic_award.missing.level | 422 | 422 | 符合 | 竞赛级别为必填项 |
| academic_award.missing.award_level | 422 | 422 | 符合 | 获奖等级为必填项 |
| academic_other.valid | 303 | 303 | 符合 | — |
| academic_other.missing.competition_name | 422 | 422 | 符合 | 竞赛完整名称为必填项 |
| academic_other.missing.competition_type | 422 | 422 | 符合 | 学科竞赛类别为必填项 |
| academic_other.missing.level | 422 | 422 | 符合 | 竞赛级别为必填项 |
| academic_other.missing.award_level | 422 | 422 | 符合 | 获奖等级为必填项 |
| academic_other.missing.other_award | 422 | 422 | 符合 | 实际奖项或名次为必填项 |
| sports_award.valid | 303 | 303 | 符合 | — |
| sports_award.missing.competition_name | 422 | 422 | 符合 | 比赛完整名称为必填项 |
| sports_award.missing.level | 422 | 422 | 符合 | 比赛级别为必填项 |
| sports_award.missing.has_award_level | 422 | 422 | 符合 | 是否有明确奖项等级为必填项 |
| sports_award.missing.award_level | 422 | 422 | 符合 | 奖项等级为必填项 |
| sports_rank.valid | 303 | 303 | 符合 | — |
| sports_rank.missing.competition_name | 422 | 422 | 符合 | 比赛完整名称为必填项 |
| sports_rank.missing.level | 422 | 422 | 符合 | 比赛级别为必填项 |
| sports_rank.missing.has_award_level | 422 | 422 | 符合 | 是否有明确奖项等级为必填项 |
| sports_rank.missing.rank | 422 | 422 | 符合 | 实际名次为必填项 |
| other_award.valid | 303 | 303 | 符合 | — |
| other_award.missing.award_name | 422 | 422 | 符合 | 荣誉完整名称为必填项 |
| other_award.missing.recognition_level | 422 | 422 | 符合 | 表彰级别为必填项 |
| other_award.missing.is_scholarship | 422 | 422 | 符合 | 是否奖学金/助学金为必填项 |
| article_academic.valid | 303 | 303 | 符合 | — |
| article_academic.missing.title | 422 | 422 | 符合 | 文章标题为必填项 |
| article_academic.missing.nature | 422 | 422 | 符合 | 文章性质为必填项 |
| article_academic.missing.publication_type | 422 | 422 | 符合 | 发表类型为必填项 |
| article_academic.missing.author_order | 422 | 422 | 符合 | 作者排序为必填项 |
| article_academic.missing.journal_name | 422 | 422 | 符合 | 期刊名称为必填项 |
| article_nonacademic.valid | 303 | 303 | 符合 | — |
| article_nonacademic.missing.title | 422 | 422 | 符合 | 文章标题为必填项 |
| article_nonacademic.missing.nature | 422 | 422 | 符合 | 文章性质为必填项 |
| article_nonacademic.missing.platform | 422 | 422 | 符合 | 发表平台为必填项 |
| article_nonacademic.missing.publication_form | 422 | 422 | 符合 | 发表形式为必填项 |
| article_nonacademic.missing.link_or_info | 422 | 422 | 符合 | 链接或发表信息为必填项 |
| social.valid | 303 | 303 | 符合 | — |
| social.missing.project_name | 422 | 422 | 符合 | 项目名称为必填项 |
| social.missing.level | 422 | 422 | 符合 | 项目级别为必填项 |
| social.missing.identity | 422 | 422 | 符合 | 本人身份为必填项 |
| social.missing.award_level_or_none | 422 | 422 | 符合 | 获奖等级或无具体等级为必填项 |
| patent.valid | 303 | 303 | 符合 | — |
| patent.missing.name | 422 | 422 | 符合 | 专利名称为必填项 |
| patent.missing.type | 422 | 422 | 符合 | 专利类型为必填项 |
| patent.missing.status | 422 | 422 | 符合 | 专利状态为必填项 |
| patent.missing.ranking | 422 | 422 | 符合 | 项目组排名为必填项 |
| patent.missing.patent_no | 422 | 422 | 符合 | 专利号或申请号为必填项 |
| certificate_cet6.valid | 303 | 303 | 符合 | — |
| certificate_cet6.missing.certificate_type | 422 | 422 | 符合 | 证书/考试类型为必填项 |
| certificate_cet6.missing.cet6_score | 422 | 422 | 符合 | CET-6 成绩为必填项 |
| certificate_computer.valid | 303 | 303 | 符合 | — |
| certificate_computer.missing.certificate_type | 422 | 422 | 符合 | 证书/考试类型为必填项 |
| certificate_computer.missing.computer_category | 422 | 422 | 符合 | 计算机专业类别为必填项 |
| certificate_computer.missing.exam_level | 422 | 422 | 符合 | 计算机考试等级为必填项 |
| certificate_language.valid | 303 | 303 | 符合 | — |
| certificate_language.missing.certificate_type | 422 | 422 | 符合 | 证书/考试类型为必填项 |
| certificate_language.missing.language_score | 422 | 422 | 符合 | 雅思/托福成绩为必填项 |
| certificate_other.valid | 303 | 303 | 符合 | — |
| certificate_other.missing.certificate_type | 422 | 422 | 符合 | 证书/考试类型为必填项 |
| certificate_other.missing.qualification_name | 422 | 422 | 符合 | 其他资格证书名称为必填项 |
| common.missing.student_name | 422 | 422 | 符合 | 姓名为 1-50 个字符 |
| common.empty.student_name | 422 | 422 | 符合 | 姓名为 1-50 个字符 |
| common.missing.student_no | 422 | 422 | 符合 | 学号为 1-30 个字符 |
| common.empty.student_no | 422 | 422 | 符合 | 学号为 1-30 个字符 |
| common.missing.result_name | 422 | 422 | 符合 | 成果名称为 1-200 个字符 |
| common.empty.result_name | 422 | 422 | 符合 | 成果名称为 1-200 个字符 |
| common.missing.obtained_date | 422 | 422 | 符合 | 取得日期格式无效 |
| common.empty.obtained_date | 422 | 422 | 符合 | 取得日期格式无效 |
| common.missing.category | 422 | 422 | 符合 | 成果类别为必填项 |
| common.empty.category | 422 | 422 | 符合 | 成果类别为必填项 |
| common.overlong.student_name | 422 | 422 | 符合 | 姓名为 1-50 个字符 |
| common.overlong.student_no | 422 | 422 | 符合 | 学号为 1-30 个字符 |
| common.overlong.result_name | 422 | 422 | 符合 | 成果名称为 1-200 个字符 |
| common.overlong.detail | 422 | 422 | 符合 | 详细说明不能超过 4000 个字符 |
| common.overlong.remark | 422 | 422 | 符合 | 备注不能超过 4000 个字符 |
| date.invalid | 422 | 422 | 符合 | 取得日期格式无效 |
| date.before | 422 | 422 | 符合 | 取得日期必须在当前学年的起止日期内 |
| date.after | 422 | 422 | 符合 | 取得日期必须在当前学年的起止日期内 |
| enum.invalid.category | 422 | 422 | 符合 | 成果类别为必填项 |
| enum.invalid.competition_type | 422 | 422 | 符合 | 学科竞赛类别取值无效 |
| enum.invalid.level | 422 | 422 | 符合 | 竞赛级别取值无效 |
| enum.invalid.award_level | 422 | 422 | 符合 | 获奖等级取值无效 |
| enum.invalid.sports_award | 422 | 422 | 符合 | 是否有明确奖项等级取值无效 |
| enum.invalid.other_award | 422 | 422 | 符合 | 是否奖学金/助学金取值无效 |
| enum.invalid.article_academic | 422 | 422 | 符合 | 文章性质取值无效 |
| enum.invalid.certificate_cet6 | 422 | 422 | 符合 | 证书/考试类型取值无效 |
| certificate_cet6.number.negative | 422 | 422 | 符合 | CET-6 成绩必须是非负数字 |
| certificate_cet6.number.not_number | 422 | 422 | 符合 | CET-6 成绩必须是非负数字 |
| certificate_cet6.number.nonfinite | 422 | 422 | 符合 | CET-6 成绩必须是非负数字 |
| certificate_language.number.negative | 422 | 422 | 符合 | 雅思/托福成绩必须是非负数字 |
| certificate_language.number.not_number | 422 | 422 | 符合 | 雅思/托福成绩必须是非负数字 |
| certificate_language.number.nonfinite | 422 | 422 | 符合 | 雅思/托福成绩必须是非负数字 |
| files.none | 422 | 422 | 符合 | 成果申报至少需要上传 1 个证明材料 |
| files.empty | 422 | 422 | 符合 | 每个附件必须大于 0 且不超过 10 MiB |
| files.extension | 422 | 422 | 符合 | 附件扩展名与文件类型不匹配 |
| files.mime | 422 | 422 | 符合 | 附件仅支持 JPG、PNG 或 PDF 格式 |
| files.mime_extension_mismatch | 422 | 422 | 符合 | 附件扩展名与文件类型不匹配 |
| files.over_10_mib | 413 | 413 | 符合 | 请求内容过大，请减少附件数量或文件大小 |
| files.eleven | 413 | 413 | 符合 | 请求内容过大，请减少附件数量或文件大小 |
| files.exactly_10_mib | 303 | 303 | 符合 | — |
| files.exactly_ten | 303 | 303 | 符合 | — |
| csrf.missing | 400 | 400 | 符合 | 请求无效，请检查填写内容后重试 |
| csrf.wrong | 400 | 400 | 符合 | 请求无效，请检查填写内容后重试 |
| session.missing | 303 | 303 | 符合 | — |
| declaration.valid | 303 | 303 | 符合 | — |
| declaration.missing.student_name | 422 | 422 | 符合 | 姓名为 1-50 个字符 |
| declaration.missing.student_no | 422 | 422 | 符合 | 学号为 1-30 个字符 |
| declaration.missing.no_result_confirm | 422 | 422 | 符合 | 请确认本学年暂无成果材料 |
| known.other_award.non_school_optional_field | 303 | 422 | 不符合（F2） | 校级荣誉类别为必填项 |

本次使用独立验收运行器完成上述探测，已有项目回归命令见最终验收报告。无 JavaScript 声明阻断是另外的浏览器用例 F1，不包含在此 HTTP 表中。临时服务在检查后停止。
