use axum::response::Html;

pub async fn newsletter_form() -> Html<&'static str> {
    Html(
        r#"<!DOCTYPE html>
     <html lang="en">
     <head>
         <meta charset="utf-8">
         <title>Send a newsletter issue</title>
     </head>
     <body>
         <h1>Send a newsletter issue</h1>
         <form action="/admin/newsletters" method="post">
             <label for="title">Title</label>
             <input id="title" name="title" type="text" required>
             <br>

             <label for="text_content">Plain-text content</label>
             <textarea id="text_content" name="text_content" 
     required></textarea>
             <br>

             <label for="html_content">HTML content</label>
             <textarea id="html_content" name="html_content" 
     required></textarea>
             <br>
        <input hidden type="text" name="idempotency_key" value="{idempotency_key}">
             <button type="submit">Send newsletter</button>
         </form>
         <p><a href="/admin/dashboard">&lt; Back</a></p>
     </body>
     </html>"#,
    )
}
