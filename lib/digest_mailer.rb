# frozen_string_literal: true

require 'sendgrid-ruby'

class DigestMailer
  FROM = { email: 'hndigest@samshadwell.com' }.freeze
  private_constant :FROM

  REPLY_TO = { email: 'hi@samshadwell.com' }.freeze
  private_constant :REPLY_TO

  def initialize(api_key:)
    @sendgrid_client = SendGrid::API.new(api_key: api_key).client
  end

  def send_mail(renderer:, recipients:)
    personalizations = recipients.map do |r|
      {
        to: [{ email: r }]
      }
    end

    mail = {
      personalizations: personalizations,
      from: FROM,
      reply_to: REPLY_TO,
      subject: renderer.subject,
      content: [
        {
          type: 'text/html',
          value: renderer.content
        }
      ]
    }

    puts 'Sending mail via Sendgrid...'
    response = @sendgrid_client.mail._('send').post(request_body: mail.to_json)
    puts "Sendgrid responded with status code #{response.status_code}"
  end
end
