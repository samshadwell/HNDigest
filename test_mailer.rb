# frozen_string_literal: true

require_relative 'lib/digest_mailer'
require_relative 'lib/digest_renderer'
require_relative 'lib/storage_adapter'

date = Time.gm(2024, 10, 26)

sa = StorageAdapter.new
digest = sa.fetch_digest(type: 'TOP_N#10', date:)
posts = digest['posts']

renderer = DigestRenderer.new(posts:, date:)
mailer = DigestMailer.new(ses_client: Aws::SES::Client.new(region: 'us-west-2'))
mailer.send_mail(
  renderer:,
  recipients: ['hi@samshadwell.com']
)
