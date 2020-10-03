# frozen_string_literal: true

require 'aws-sdk-ses'

class DigestMailer
  SES_RECIPIENT_LIMIT = 50
  private_constant :SES_RECIPIENT_LIMIT

  FROM = 'hndigest@samshadwell.com'
  private_constant :FROM

  REPLY_TO = 'hi@samshadwell.com'
  private_constant :REPLY_TO

  ENCODING = 'UTF-8'
  private_constant :ENCODING

  def initialize(ses_client:)
    @ses_client = ses_client
  end

  def send_mail(renderer:, recipients:)
    recipients.each_slice(SES_RECIPIENT_LIMIT) do |recipients_slice|
      puts 'Sending mail via SES...'
      response = ses_client.send_email({
        source: FROM,
        destination: {
          bcc_addresses: recipients_slice
        },
        reply_to_addresses: [REPLY_TO],
        return_path: REPLY_TO,
        message: {
          subject: {
            data: renderer.subject,
            charset: ENCODING,
          },
          body: {
            html: {
              data: renderer.content,
              charset: ENCODING,
            }
          }
        }
      })
      puts "Success! message_id=#{response.message_id}"
    end
  end
end
