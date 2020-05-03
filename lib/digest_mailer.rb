# frozen_string_literal: true

require 'sendgrid-ruby'

class DigestMailer
  FROM = SendGrid::Email.new(email: 'hndigest@samshadwell.com')
  private_constant :FROM

  def initialize(api_key:)
    @sendgrid_client = SendGrid::API.new(api_key: api_key).client
  end

  def send_mail(renderer:, recipients:)
    recipients.each do |recipient|
      to = SendGrid::Email.new(email: recipient)
      subject = renderer.subject
      content = SendGrid::Content.new(
        type: 'text/html',
        value: renderer.content
      )
      mail = SendGrid::Mail.new(FROM, subject, to, content)

      @sendgrid_client.mail._('send').post(request_body: mail.to_json)
    end
  end
end
