# frozen_string_literal: true

require_relative 'lib/digest_mailer'
require_relative 'lib/digest_renderer'
require_relative 'lib/storage_adapter'

date = Time.gm(2020, 5, 2)

sa = StorageAdapter.new
digest = sa.fetch_digest(type: 'TOP_N#10', date: date)
posts = digest['posts']

renderer = DigestRenderer.new(posts: posts, date: date)

mailer = DigestMailer.new(api_key: ENV['SENDGRID_API_KEY'])
mailer.send_mail(
  renderer: renderer,
  recipients: ['test1@samshadwell.com', 'test2@samshadwell.com']
)
