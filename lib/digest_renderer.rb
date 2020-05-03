# frozen_string_literal: true

require 'erb'

class DigestRenderer
  TEMPLATE = %(
    Your daily Hacker News digest:
    <br>
    <br>
    <% for @post in @posts %>
      <p>
        <%= @post['title'] %>
        <br>
        <% if @post['url'] %>
          <a href="<%= @post['url'] %>">
            link
          </a> -
        <% end %>
        <a href="https://news.ycombinator.com/item?id=<%= @post['objectID'] %>">
          comments
        </a>
      </p>
    <% end %>
    <br>
    <br>
    To unsubscribe, reply to this email.
  )
  private_constant :TEMPLATE

  def initialize(posts:, date:)
    @date = date
    @posts = posts
  end

  def subject
    "Hacker News Digest for #{@date.getutc.strftime('%b %-d, %Y')}"
  end

  def content
    ERB.new(TEMPLATE, trim_mode: '>-').result(binding)
  end
end
